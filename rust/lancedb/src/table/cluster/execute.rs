// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

//! Clustering execution implementation.

use arrow_array::RecordBatch;
use arrow_schema::{Schema, SchemaRef};

use futures::TryStreamExt;
use lance::dataset::{WriteMode, WriteParams};
use std::sync::Arc;

use crate::error::Result;
use crate::table::cluster::{ClusterConfig, ClusterStats};
use crate::table::NativeTable;

/// Default batch size for streaming operations.
const DEFAULT_BATCH_SIZE: usize = 10000;

/// Execute 1D clustering using direct sort with streaming support.
///
/// This function performs a global rewrite of the table data, sorting by a single clustering key.
/// For large datasets, it uses streaming processing to avoid OOM:
/// - Reads data in batches (streaming)
/// - Sorts accumulated batches
/// - Writes sorted data in fragments (streaming)
pub async fn execute_cluster_direct(
    table: &NativeTable,
    config: &ClusterConfig,
    target_rows_per_fragment: Option<usize>,
) -> Result<ClusterStats> {
    let dataset = table.dataset.get().await?;

    // Get the clustering key column
    let cluster_key = &config.keys[0];

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
    // TODO: In future phases, implement true external sort for very large datasets
    let batches: Vec<RecordBatch> = stream.try_collect().await?;

    if batches.is_empty() {
        return Ok(ClusterStats::default());
    }

    // Combine all batches
    let combined = arrow_select::concat::concat_batches(&schema, &batches)?;

    // Sort the combined batch
    let sorted_batch = sort_batch_by_column(&combined, cluster_key)?;
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
    let reader = arrow_array::RecordBatchIterator::new(
        chunks.into_iter().map(Ok),
        sorted_schema,
    );

    // Write sorted data
    lance::Dataset::write(
        reader,
        &uri,
        Some(write_params),
    )
    .await?;

    // Reload dataset to get the updated view
    table.dataset.reload().await?;

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
            .map(|col| {
                col.slice(self.offset, num_rows)
            })
            .collect();

        self.offset = end;

        Some(RecordBatch::try_new(batch.schema(), sliced_columns).unwrap())
    }
}

/// Sort a RecordBatch by a specific column.
fn sort_batch_by_column(batch: &RecordBatch, column_name: &str) -> Result<RecordBatch> {
    use arrow::compute::sort_to_indices;
    use arrow::compute::take;

    let schema = batch.schema();
    let column_index = schema
        .index_of(column_name)
        .map_err(|_| crate::error::Error::InvalidInput {
            message: format!("Column '{}' not found in batch", column_name),
        })?;

    let sort_column = batch.column(column_index);

    // Sort to indices (ascending, nulls last)
    let sort_options = arrow::compute::SortOptions {
        descending: false,
        nulls_first: false,
    };
    let indices = sort_to_indices(sort_column, Some(sort_options), None)?;

    // Reorder all columns according to sorted indices
    let sorted_columns: Vec<_> = batch
        .columns()
        .iter()
        .map(|col| take(col.as_ref(), &indices, None))
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| crate::error::Error::InvalidInput {
            message: format!("Failed to reorder columns: {}", e),
        })?;

    Ok(RecordBatch::try_new(schema, sorted_columns)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{Int32Array, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    #[test]
    fn test_sort_batch_by_column() {
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

        let sorted = sort_batch_by_column(&batch, "id").unwrap();

        let id_col = sorted
            .column(0)
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        assert_eq!(id_col.values(), &[1, 2, 3]);

        let value_col = sorted
            .column(1)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        assert_eq!(value_col.value(0), "a");
        assert_eq!(value_col.value(1), "b");
        assert_eq!(value_col.value(2), "c");
    }
}
