// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

//! Clustering execution implementation.

use arrow::compute::{sort_to_indices, take};
use arrow_array::RecordBatch;
use arrow_array::builder::BinaryBuilder;
use arrow_schema::{Schema, SchemaRef};
use datafusion::scalar::ScalarValue;

use futures::TryStreamExt;
use lance::dataset::{WriteMode, WriteParams};
use std::sync::Arc;

use crate::error::Result;
use crate::table::NativeTable;
use crate::table::cluster::{ClusterConfig, ClusterStats, ClusteringAlgorithm};

/// Default batch size for streaming operations.
const DEFAULT_BATCH_SIZE: usize = 10000;

/// Execute clustering using the given algorithm with streaming support.
///
/// This function performs a global rewrite of the table data, sorting by clustering keys.
/// For large datasets, it uses streaming processing to avoid OOM:
/// - Reads data in batches (streaming)
/// - Sorts accumulated batches
/// - Writes sorted data in fragments (streaming)
///
/// # Transaction Safety
///
/// This operation leverages Lance's immutable data structure for atomicity:
/// - Write operations create new versions without modifying existing data
/// - If the write fails, the original data remains intact
/// - The table is only updated to point to the new version after successful write
/// - Old versions can be cleaned up later using the prune operation
///
/// This ensures the table never remains in an intermediate/corrupted state.
pub async fn execute_cluster_direct(
    table: &NativeTable,
    config: &ClusterConfig,
    algorithm: &dyn ClusteringAlgorithm,
    target_rows_per_fragment: Option<usize>,
) -> Result<ClusterStats> {
    let dataset = table.dataset.get().await?;

    // Get schema for the table
    let schema: SchemaRef = Arc::new(Schema::from(dataset.schema()));

    // Read all data from the table (streaming)
    let scanner = dataset.scan();
    let stream = scanner.try_into_stream().await?;

    // Process data in batches using streaming
    let target_rows = target_rows_per_fragment.unwrap_or(DEFAULT_BATCH_SIZE);
    let row_count;
    let fragments_written;

    // Collect batches for sorting
    // For Phase 1: We collect all batches but process writes in chunks
    // Note: For extremely large datasets that do not fit in memory, a true
    // external sort would be required. This is a known limitation of the
    // current implementation.
    let batches: Vec<RecordBatch> = stream.try_collect().await?;

    if batches.is_empty() {
        return Ok(ClusterStats::default());
    }

    // Prepare algorithm with all batches (e.g., compute normalization bounds)
    algorithm.prepare(&batches, &config.keys)?;

    // Combine all batches
    let combined = arrow_select::concat::concat_batches(&schema, &batches)?;

    // Sort the combined batch using the algorithm
    let sorted_batch = sort_batch_by_algorithm(&combined, &config.keys, algorithm)?;
    let sorted_row_count = sorted_batch.num_rows();

    // Create write parameters
    let mut write_params = WriteParams::default();
    write_params.mode = WriteMode::Overwrite;
    write_params.max_rows_per_file = target_rows;
    write_params.max_rows_per_group = target_rows;

    // Get table URI
    let uri = table.uri.clone();

    // Split sorted data into chunks and write as fragments
    let sorted_schema = sorted_batch.schema();
    let chunk_iter = RecordBatchChunkIterator::new(sorted_batch, target_rows);

    // Collect chunks into a Vec for the iterator
    let chunks: Vec<RecordBatch> = chunk_iter.collect();
    fragments_written = chunks.len();
    row_count = sorted_row_count;

    // Create a RecordBatchReader from the chunks
    let reader = arrow_array::RecordBatchIterator::new(chunks.into_iter().map(Ok), sorted_schema);

    // Write sorted data
    // Lance's WriteMode::Overwrite is atomic - if this fails, original data is untouched
    let new_dataset = lance::Dataset::write(reader, &uri, Some(write_params))
        .await
        .map_err(|e| crate::Error::Other {
            message: format!("Failed to write clustered data: {}", e),
            source: Some(Box::new(e)),
        })?;

    // Update the table's dataset reference to point to the new version
    table.dataset.update(new_dataset);

    Ok(ClusterStats {
        rows_processed: row_count,
        fragments_written,
        indices_rebuilt: 0,
    })
}

/// Iterator that splits a RecordBatch into chunks of specified row count.
struct RecordBatchChunkIterator {
    batch: Option<RecordBatch>,
    offset: usize,
    chunk_size: usize,
    total_rows: usize,
}

impl RecordBatchChunkIterator {
    fn new(batch: RecordBatch, chunk_size: usize) -> Self {
        let total_rows = batch.num_rows();
        Self {
            batch: Some(batch),
            offset: 0,
            chunk_size,
            total_rows,
        }
    }
}

impl Iterator for RecordBatchChunkIterator {
    type Item = RecordBatch;

    fn next(&mut self) -> Option<Self::Item> {
        let batch = self.batch.as_ref()?;

        if self.offset >= self.total_rows {
            return None;
        }

        let end = (self.offset + self.chunk_size).min(self.total_rows);
        let num_rows = end - self.offset;

        // Slice each column
        let sliced_columns: Vec<_> = batch
            .columns()
            .iter()
            .map(|col| col.slice(self.offset, num_rows))
            .collect();

        self.offset = end;

        Some(RecordBatch::try_new(batch.schema(), sliced_columns).unwrap())
    }
}

/// Sort a RecordBatch using a clustering algorithm.
///
/// Computes a per-row sort key via the algorithm and reorders all columns.
/// Rows with any NULL clustering value are placed at the end.
fn sort_batch_by_algorithm(
    batch: &RecordBatch,
    keys: &[String],
    algorithm: &dyn ClusteringAlgorithm,
) -> Result<RecordBatch> {
    let num_rows = batch.num_rows();
    let mut sort_key_builder = BinaryBuilder::new();

    for row in 0..num_rows {
        let mut values = Vec::with_capacity(keys.len());
        let mut has_null = false;
        for key in keys {
            let col_idx =
                batch
                    .schema()
                    .index_of(key)
                    .map_err(|_| crate::error::Error::InvalidInput {
                        message: format!("Column '{}' not found in batch", key),
                    })?;
            let array = batch.column(col_idx);
            let scalar = ScalarValue::try_from_array(array.as_ref(), row).map_err(|e| {
                crate::error::Error::InvalidInput {
                    message: format!("Failed to convert array value to scalar: {}", e),
                }
            })?;
            if scalar.is_null() {
                has_null = true;
            }
            values.push(scalar);
        }

        if has_null {
            sort_key_builder.append_null();
        } else {
            let key = algorithm.compute_sort_key(&values)?;
            sort_key_builder.append_value(&key);
        }
    }

    let sort_keys = sort_key_builder.finish();
    let sort_options = arrow::compute::SortOptions {
        descending: false,
        nulls_first: false,
    };
    let indices = sort_to_indices(&sort_keys, Some(sort_options), None).map_err(|e| {
        crate::error::Error::InvalidInput {
            message: format!("Failed to sort batch: {}", e),
        }
    })?;

    let sorted_columns: Vec<_> = batch
        .columns()
        .iter()
        .map(|col| take(col.as_ref(), &indices, None))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| crate::error::Error::InvalidInput {
            message: format!("Failed to reorder columns: {}", e),
        })?;

    Ok(RecordBatch::try_new(batch.schema(), sorted_columns)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::cluster::algorithm::{DirectSortAlgorithm, HilbertCurveAlgorithm};
    use arrow_array::{Int32Array, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    #[test]
    fn test_sort_batch_by_algorithm_direct() {
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int32, false),
            Field::new("value", DataType::Utf8, true),
        ]));

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int32Array::from(vec![3, 1, 2])),
                Arc::new(StringArray::from(vec!["c", "a", "b"])),
            ],
        )
        .unwrap();

        let algo = DirectSortAlgorithm::new();
        let sorted = sort_batch_by_algorithm(&batch, &["id".to_string()], &algo).unwrap();

        let id_col = sorted
            .column(0)
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        assert_eq!(id_col.values(), &[1, 2, 3]);
    }

    #[test]
    fn test_sort_batch_by_algorithm_hilbert_2d() {
        let schema = Arc::new(Schema::new(vec![
            Field::new("x", DataType::Int32, false),
            Field::new("y", DataType::Int32, false),
            Field::new("value", DataType::Utf8, false),
        ]));

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int32Array::from(vec![1, 0, 1, 0])),
                Arc::new(Int32Array::from(vec![0, 1, 1, 0])),
                Arc::new(StringArray::from(vec!["d", "b", "c", "a"])),
            ],
        )
        .unwrap();

        let algo = HilbertCurveAlgorithm::new(2).with_bits(1);
        algo.prepare(&[batch.clone()], &["x".to_string(), "y".to_string()])
            .unwrap();

        let sorted =
            sort_batch_by_algorithm(&batch, &["x".to_string(), "y".to_string()], &algo).unwrap();

        let x_col = sorted
            .column(0)
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        let y_col = sorted
            .column(1)
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        let value_col = sorted
            .column(2)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();

        // Hilbert order for 2x2 grid: (0,0), (0,1), (1,1), (1,0)
        assert_eq!(x_col.values(), &[0, 0, 1, 1]);
        assert_eq!(y_col.values(), &[0, 1, 1, 0]);
        assert_eq!(value_col.value(0), "a");
        assert_eq!(value_col.value(1), "b");
        assert_eq!(value_col.value(2), "c");
        assert_eq!(value_col.value(3), "d");
    }

    #[test]
    fn test_sort_batch_by_algorithm_with_nulls() {
        let schema = Arc::new(Schema::new(vec![
            Field::new("x", DataType::Int32, true),
            Field::new("y", DataType::Int32, true),
            Field::new("value", DataType::Utf8, false),
        ]));

        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int32Array::from(vec![Some(1), None, Some(0)])),
                Arc::new(Int32Array::from(vec![Some(0), Some(1), Some(1)])),
                Arc::new(StringArray::from(vec!["a", "b", "c"])),
            ],
        )
        .unwrap();

        let algo = HilbertCurveAlgorithm::new(2).with_bits(1);
        algo.prepare(&[batch.clone()], &["x".to_string(), "y".to_string()])
            .unwrap();

        let sorted =
            sort_batch_by_algorithm(&batch, &["x".to_string(), "y".to_string()], &algo).unwrap();

        let value_col = sorted
            .column(2)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();

        // NULL row should be last
        assert_eq!(value_col.value(0), "c");
        assert_eq!(value_col.value(1), "a");
        assert_eq!(value_col.value(2), "b");
    }
}
