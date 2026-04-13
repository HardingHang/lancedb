// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

//! Clustering module for multi-dimensional data clustering.
//!
//! This module provides functionality to cluster table data by specified keys,
//! optimizing range query performance through physical data layout.


use arrow_schema::SchemaRef;
use datafusion::scalar::ScalarValue;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub mod algorithm;
pub mod execute;

/// Configuration for table clustering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClusterConfig {
    /// Clustering key column names (1-4 columns supported)
    pub keys: Vec<String>,
    /// Algorithm name ("direct" for 1D, "hilbert" for 2-4D)
    pub algorithm: String,
    /// Optional algorithm parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algorithm_params: Option<serde_json::Value>,
}

impl ClusterConfig {
    /// The key used to store cluster config in schema metadata.
    pub const SCHEMA_METADATA_KEY: &'static str = "lancedb.cluster.config";

    /// Create a new cluster configuration.
    pub fn new(keys: Vec<String>) -> Self {
        let algorithm = if keys.len() == 1 {
            "direct".to_string()
        } else {
            "hilbert".to_string()
        };
        Self {
            keys,
            algorithm,
            algorithm_params: None,
        }
    }

    /// Create a new cluster configuration with a specific algorithm.
    pub fn with_algorithm(mut self, algorithm: impl Into<String>) -> Self {
        self.algorithm = algorithm.into();
        self
    }

    /// Parse cluster config from schema metadata.
    pub fn from_schema_metadata(metadata: &std::collections::HashMap<String, String>) -> Option<Self> {
        metadata
            .get(Self::SCHEMA_METADATA_KEY)
            .and_then(|value| serde_json::from_str(value).ok())
    }

    /// Serialize to JSON string for storage in schema metadata.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| Error::Other {
            message: format!("Failed to serialize cluster config: {}", e),
            source: Some(Box::new(e)),
        })
    }

    /// Validate the configuration against the given schema.
    pub fn validate(&self, schema: &SchemaRef) -> Result<()> {
        // Check number of clustering keys
        if self.keys.is_empty() {
            return Err(Error::InvalidInput {
                message: "Clustering keys cannot be empty".to_string(),
            });
        }
        if self.keys.len() > 4 {
            return Err(Error::InvalidInput {
                message: format!(
                    "Clustering keys count {} exceeds maximum of 4",
                    self.keys.len()
                ),
            });
        }

        // Check for duplicate column names
        let mut seen = std::collections::HashSet::new();
        for key in &self.keys {
            if key.is_empty() {
                return Err(Error::InvalidInput {
                    message: "Empty clustering key column name".to_string(),
                });
            }
            if !seen.insert(key) {
                return Err(Error::InvalidInput {
                    message: format!("Duplicate clustering key column: {}", key),
                });
            }
        }

        // Check column existence and type
        for key in &self.keys {
            let field = schema.field_with_name(key).map_err(|_| Error::InvalidInput {
                message: format!("Clustering key column '{}' not found in schema", key),
            })?;

            if !Self::is_supported_type(field.data_type()) {
                return Err(Error::InvalidInput {
                    message: format!(
                        "Type '{}' is not supported for clustering",
                        field.data_type()
                    ),
                });
            }
        }

        Ok(())
    }

    /// Check if a data type is supported for clustering.
    fn is_supported_type(data_type: &arrow_schema::DataType) -> bool {
        use arrow_schema::DataType;
        matches!(
            data_type,
            DataType::Int8
                | DataType::Int16
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt8
                | DataType::UInt16
                | DataType::UInt32
                | DataType::UInt64
                | DataType::Float32
                | DataType::Float64
                | DataType::Timestamp(_, _)
                | DataType::Date32
                | DataType::Date64
        )
    }
}

/// Trait for clustering algorithms.
pub trait ClusteringAlgorithm: Send + Sync {
    /// Get the algorithm name.
    fn name(&self) -> &str;

    /// Compute sort key for a row given its clustering column values.
    /// Returns bytes that can be compared for ordering.
    fn compute_sort_key(&self, values: &[ScalarValue]) -> Result<Vec<u8>>;
}

/// Statistics returned from a clustering operation.
#[derive(Debug, Default)]
pub struct ClusterStats {
    /// Number of rows processed.
    pub rows_processed: usize,
    /// Number of fragments written.
    pub fragments_written: usize,
    /// Number of indices rebuilt.
    pub indices_rebuilt: usize,
}

/// Validate and create a cluster configuration from column names.
pub fn validate_cluster_keys(
    keys: &[String],
    schema: &SchemaRef,
) -> Result<ClusterConfig> {
    let config = ClusterConfig::new(keys.to_vec());
    config.validate(schema)?;
    Ok(config)
}

pub use execute::execute_cluster_direct;

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    fn create_test_schema() -> SchemaRef {
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int64, false),
            Field::new("timestamp", DataType::Int64, false),
            Field::new("value", DataType::Float64, true),
            Field::new("name", DataType::Utf8, true),
        ]))
    }

    #[test]
    fn test_cluster_config_validation_success() {
        let schema = create_test_schema();
        let config = ClusterConfig::new(vec!["id".to_string()]);
        assert!(config.validate(&schema).is_ok());

        let config = ClusterConfig::new(vec!["id".to_string(), "timestamp".to_string()]);
        assert!(config.validate(&schema).is_ok());
    }

    #[test]
    fn test_cluster_config_validation_empty_keys() {
        let schema = create_test_schema();
        let config = ClusterConfig::new(vec![]);
        let result = config.validate(&schema);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_cluster_config_validation_too_many_keys() {
        let schema = create_test_schema();
        let config = ClusterConfig::new(vec![
            "id".to_string(),
            "timestamp".to_string(),
            "value".to_string(),
            "name".to_string(),
            "extra".to_string(),
        ]);
        let result = config.validate(&schema);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds maximum of 4"));
    }

    #[test]
    fn test_cluster_config_validation_duplicate_keys() {
        let schema = create_test_schema();
        let config = ClusterConfig::new(vec!["id".to_string(), "id".to_string()]);
        let result = config.validate(&schema);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Duplicate clustering key"));
    }

    #[test]
    fn test_cluster_config_validation_column_not_found() {
        let schema = create_test_schema();
        let config = ClusterConfig::new(vec!["nonexistent".to_string()]);
        let result = config.validate(&schema);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not found in schema"));
    }

    #[test]
    fn test_cluster_config_validation_unsupported_type() {
        let schema = create_test_schema();
        let config = ClusterConfig::new(vec!["name".to_string()]); // Utf8 is not supported
        let result = config.validate(&schema);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not supported for clustering"));
    }

    #[test]
    fn test_cluster_config_serialization() {
        let config = ClusterConfig::new(vec!["id".to_string(), "timestamp".to_string()])
            .with_algorithm("hilbert");

        let json = config.to_json().unwrap();
        let metadata = std::collections::HashMap::from([(
            ClusterConfig::SCHEMA_METADATA_KEY.to_string(),
            json,
        )]);

        let parsed = ClusterConfig::from_schema_metadata(&metadata);
        assert_eq!(parsed, Some(config));
    }

    #[test]
    fn test_cluster_config_auto_algorithm_selection() {
        let config1 = ClusterConfig::new(vec!["id".to_string()]);
        assert_eq!(config1.algorithm, "direct");

        let config2 = ClusterConfig::new(vec!["id".to_string(), "timestamp".to_string()]);
        assert_eq!(config2.algorithm, "hilbert");
    }


    // Integration tests for Phase 0

    use crate::connect;
    use crate::query::ExecutableQuery;
    use crate::table::OptimizeAction;
    use arrow_array::{Array, RecordBatch};
    use futures::TryStreamExt;

    #[tokio::test]
    async fn test_create_table_with_cluster_config() {
        let conn = connect("memory://").execute().await.unwrap();

        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, false)]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(arrow_array::Int64Array::from(vec![3, 1, 2]))],
        )
        .unwrap();

        let table = conn
            .create_table("test_cluster", batch)
            .cluster_by(&["id"])
            .execute()
            .await
            .unwrap();

        let config = table.cluster_config().await.unwrap();
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.keys, vec!["id"]);
        assert_eq!(config.algorithm, "direct");
    }

    #[tokio::test]
    async fn test_cluster_config_returns_none_for_non_clustered_table() {
        let conn = connect("memory://").execute().await.unwrap();

        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, false)]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(arrow_array::Int64Array::from(vec![1, 2, 3]))],
        )
        .unwrap();

        let table = conn
            .create_table("test_no_cluster", batch)
            .execute()
            .await
            .unwrap();

        let config = table.cluster_config().await.unwrap();
        assert!(config.is_none());
    }

    #[tokio::test]
    async fn test_optimize_cluster_not_supported_for_unconfigured_table() {
        let conn = connect("memory://").execute().await.unwrap();

        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, false)]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(arrow_array::Int64Array::from(vec![1, 2, 3]))],
        )
        .unwrap();

        let table = conn
            .create_table("test_cluster_error", batch)
            .execute()
            .await
            .unwrap();

        let result = table
            .optimize(OptimizeAction::Cluster {
                full: true,
                target_rows_per_fragment: None,
            })
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("does not have clustering configured"));
    }

    #[tokio::test]
    async fn test_optimize_cluster_incremental_not_supported() {
        let conn = connect("memory://").execute().await.unwrap();

        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, false)]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(arrow_array::Int64Array::from(vec![1, 2, 3]))],
        )
        .unwrap();

        let table = conn
            .create_table("test_incremental", batch)
            .cluster_by(&["id"])
            .execute()
            .await
            .unwrap();

        let result = table
            .optimize(OptimizeAction::Cluster {
                full: false,
                target_rows_per_fragment: None,
            })
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Incremental clustering not supported"));
    }

    #[tokio::test]
    async fn test_create_table_with_multidimensional_cluster_not_supported() {
        let conn = connect("memory://").execute().await.unwrap();

        let schema = Arc::new(Schema::new(vec![
            Field::new("x", DataType::Int64, false),
            Field::new("y", DataType::Int64, false),
        ]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(arrow_array::Int64Array::from(vec![1, 2, 3])),
                Arc::new(arrow_array::Int64Array::from(vec![4, 5, 6])),
            ],
        )
        .unwrap();

        let table = conn
            .create_table("test_2d", batch)
            .cluster_by(&["x", "y"])
            .execute()
            .await
            .unwrap();

        let config = table.cluster_config().await.unwrap();
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(config.keys, vec!["x", "y"]);
        assert_eq!(config.algorithm, "hilbert");

        let result = table
            .optimize(OptimizeAction::Cluster {
                full: true,
                target_rows_per_fragment: None,
            })
            .await;

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Multi-dimensional clustering"));
        assert!(err_msg.contains("not yet implemented"));
    }

    #[tokio::test]
    async fn test_cluster_empty_table() {
        let conn = connect("memory://").execute().await.unwrap();

        // Create an empty table with clustering config
        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, false)]));
        let empty_batch = RecordBatch::try_new(
            schema.clone(),
            vec![Arc::new(arrow_array::Int64Array::from(Vec::<i64>::new()))],
        )
        .unwrap();

        let table = conn
            .create_table("test_empty", empty_batch)
            .cluster_by(&["id"])
            .execute()
            .await
            .unwrap();

        // Verify table is empty
        let count = table.count_rows(None).await.unwrap();
        assert_eq!(count, 0);

        // Run cluster operation - should succeed and return empty stats
        let stats = table
            .optimize(OptimizeAction::Cluster {
                full: true,
                target_rows_per_fragment: None,
            })
            .await
            .unwrap();

        // Verify empty stats
        let cluster_stats = stats.cluster.unwrap();
        assert_eq!(cluster_stats.rows_processed, 0);
        assert_eq!(cluster_stats.fragments_written, 0);

        // Verify table is still empty after clustering
        let count_after = table.count_rows(None).await.unwrap();
        assert_eq!(count_after, 0);
    }

    #[tokio::test]
    async fn test_cluster_all_null_values() {
        let conn = connect("memory://").execute().await.unwrap();

        // Create a table with all NULL values in the clustering key
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int64, true),  // Nullable
            Field::new("value", DataType::Utf8, false),
        ]));
        let batch = RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(arrow_array::Int64Array::from(vec![None, None, None])),
                Arc::new(arrow_array::StringArray::from(vec!["a", "b", "c"])),
            ],
        )
        .unwrap();

        let table = conn
            .create_table("test_null_cluster", batch)
            .cluster_by(&["id"])
            .execute()
            .await
            .unwrap();

        // Run cluster operation - should succeed
        let stats = table
            .optimize(OptimizeAction::Cluster {
                full: true,
                target_rows_per_fragment: None,
            })
            .await
            .unwrap();

        // All rows should be processed (they exist, just have NULL cluster key)
        let cluster_stats = stats.cluster.unwrap();
        assert_eq!(cluster_stats.rows_processed, 3);

        // Query data to verify order is preserved (or at least data is intact)
        let results = table.query().execute().await.unwrap();
        let batches: Vec<_> = results.try_collect().await.unwrap();
        let result_batch = &batches[0];

        // Verify all rows are present
        assert_eq!(result_batch.num_rows(), 3);

        // Verify values are intact (order may vary if sorted, but data should be there)
        let value_col = result_batch
            .column(1)
            .as_any()
            .downcast_ref::<arrow_array::StringArray>()
            .unwrap();
        let values: Vec<_> = (0..value_col.len()).map(|i| value_col.value(i)).collect();
        assert!(values.contains(&"a"));
        assert!(values.contains(&"b"));
        assert!(values.contains(&"c"));
    }
}
