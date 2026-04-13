// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

//! Clustering execution implementation.

use arrow_array::RecordBatch;

use futures::TryStreamExt;
use lance::dataset::WriteParams;

use crate::error::Result;
use crate::table::cluster::{ClusterConfig, ClusterStats};
use crate::table::NativeTable;

/// Execute 1D clustering using direct sort.
///
/// This function performs a global rewrite of the table data, sorting by a single clustering key.
pub async fn execute_cluster_direct(
    table: &NativeTable,
    config: &ClusterConfig,
    target_rows_per_fragment: Option<usize>,
) -> Result<ClusterStats> {
    let dataset = table.dataset.get().await?;

    // Get the clustering key column
    let cluster_key = &config.keys[0];

    // Read all data from the table
    let scanner = dataset.scan();
    let stream = scanner.try_into_stream().await?;

    // Collect all batches and sort them
    let batches: Vec<RecordBatch> = stream.try_collect().await?;

    if batches.is_empty() {
        return Ok(ClusterStats::default());
    }

    // Combine all batches into one (for small datasets in Phase 0)
    // TODO: In Phase 1, implement streaming sort for large datasets
    let combined = arrow_select::concat::concat_batches(&batches[0].schema(), &batches)?;

    // Sort by clustering key
    let sorted_batch = sort_batch_by_column(&combined, cluster_key)?;

    // Write sorted data back
    let row_count = sorted_batch.num_rows();

    // Create write parameters
    let mut write_params = WriteParams::default();
    if let Some(target_rows) = target_rows_per_fragment {
        write_params.max_rows_per_file = target_rows;
        write_params.max_rows_per_group = target_rows;
    }

    // For Phase 0, simplified write: delete old data and write new sorted data
    // TODO: In Phase 2, implement proper index rebuild

    // Get table URI
    let uri = table.uri.clone();

    // Create a RecordBatchReader from the sorted batch
    let schema = sorted_batch.schema();
    let reader = arrow_array::RecordBatchIterator::new(
        vec![Ok(sorted_batch)].into_iter(),
        schema,
    );

    // Create a new dataset with sorted data
    let _ = lance::Dataset::write(
        reader,
        &uri,
        Some(write_params),
    )
    .await?;

    // Update dataset reference
    // Note: In Phase 0, this is simplified. Phase 2 will handle proper index rebuild.

    Ok(ClusterStats {
        rows_processed: row_count,
        fragments_written: 1, // Simplified for Phase 0
        indices_rebuilt: 0,   // Not implemented in Phase 0
    })
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
