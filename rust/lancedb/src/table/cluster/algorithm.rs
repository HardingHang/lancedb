// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

//! Clustering algorithms implementation.

use datafusion::scalar::ScalarValue;

use crate::error::{Error, Result};
use crate::table::cluster::ClusteringAlgorithm;

/// Direct sort algorithm for single-column clustering.
///
/// This algorithm simply sorts data by a single column value.
/// It's the most efficient choice for 1-dimensional clustering.
pub struct DirectSortAlgorithm;

impl DirectSortAlgorithm {
    /// Create a new direct sort algorithm.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DirectSortAlgorithm {
    fn default() -> Self {
        Self::new()
    }
}

impl ClusteringAlgorithm for DirectSortAlgorithm {
    fn name(&self) -> &str {
        "direct"
    }

    fn compute_sort_key(&self, values: &[ScalarValue]) -> Result<Vec<u8>> {
        if values.len() != 1 {
            return Err(Error::InvalidInput {
                message: format!(
                    "Direct sort requires exactly 1 value, got {}",
                    values.len()
                ),
            });
        }
        Ok(scalar_value_to_sort_key(&values[0]))
    }
}

/// Convert a ScalarValue to a sortable byte representation.
///
/// The byte representation is designed such that lexicographic comparison
/// of bytes matches the natural ordering of the original values.
pub fn scalar_value_to_sort_key(value: &ScalarValue) -> Vec<u8> {
    use ScalarValue::*;

    match value {
        // Signed integers: flip sign bit so negative < positive
        Int8(v) => {
            let mut bytes = Vec::with_capacity(1);
            let val = v.unwrap_or(i8::MIN);
            bytes.push((val as u8) ^ 0x80);
            bytes
        }
        Int16(v) => {
            let mut bytes = Vec::with_capacity(2);
            let v = v.unwrap_or(i16::MIN);
            bytes.extend_from_slice(&(v ^ (1 << 15)).to_be_bytes());
            bytes
        }
        Int32(v) => {
            let mut bytes = Vec::with_capacity(4);
            let v = v.unwrap_or(i32::MIN);
            bytes.extend_from_slice(&(v ^ (1 << 31)).to_be_bytes());
            bytes
        }
        Int64(v) => {
            let mut bytes = Vec::with_capacity(8);
            let v = v.unwrap_or(i64::MIN);
            bytes.extend_from_slice(&(v ^ (1i64 << 63)).to_be_bytes());
            bytes
        }

        // Unsigned integers: direct big-endian representation
        UInt8(v) => {
            let mut bytes = Vec::with_capacity(1);
            bytes.push(v.unwrap_or(0));
            bytes
        }
        UInt16(v) => {
            let mut bytes = Vec::with_capacity(2);
            bytes.extend_from_slice(&v.unwrap_or(0).to_be_bytes());
            bytes
        }
        UInt32(v) => {
            let mut bytes = Vec::with_capacity(4);
            bytes.extend_from_slice(&v.unwrap_or(0).to_be_bytes());
            bytes
        }
        UInt64(v) => {
            let mut bytes = Vec::with_capacity(8);
            bytes.extend_from_slice(&v.unwrap_or(0).to_be_bytes());
            bytes
        }

        // Floats: handle NaN and normalize for comparison
        Float32(v) => {
            let mut bytes = Vec::with_capacity(4);
            let f = v.unwrap_or(f32::NAN);
            let bits = if f.is_nan() {
                // NaN comes last
                u32::MAX
            } else if f >= 0.0 {
                // Positive: flip sign bit
                f.to_bits() ^ (1 << 31)
            } else {
                // Negative: flip all bits (reversed order for negatives)
                !f.to_bits()
            };
            bytes.extend_from_slice(&bits.to_be_bytes());
            bytes
        }
        Float64(v) => {
            let mut bytes = Vec::with_capacity(8);
            let f = v.unwrap_or(f64::NAN);
            let bits = if f.is_nan() {
                // NaN comes last
                u64::MAX
            } else if f >= 0.0 {
                // Positive: flip sign bit
                f.to_bits() ^ (1 << 63)
            } else {
                // Negative: flip all bits (reversed order for negatives)
                !f.to_bits()
            };
            bytes.extend_from_slice(&bits.to_be_bytes());
            bytes
        }

        // Timestamps: store as i64 (microseconds or nanoseconds)
        TimestampNanosecond(v, _) => {
            let mut bytes = Vec::with_capacity(8);
            let v = v.unwrap_or(i64::MIN);
            bytes.extend_from_slice(&(v ^ (1i64 << 63)).to_be_bytes());
            bytes
        }
        TimestampMicrosecond(v, _) => {
            let mut bytes = Vec::with_capacity(8);
            let v = v.unwrap_or(i64::MIN);
            bytes.extend_from_slice(&(v ^ (1i64 << 63)).to_be_bytes());
            bytes
        }
        TimestampMillisecond(v, _) => {
            let mut bytes = Vec::with_capacity(8);
            let v = v.unwrap_or(i64::MIN);
            bytes.extend_from_slice(&(v ^ (1i64 << 63)).to_be_bytes());
            bytes
        }
        TimestampSecond(v, _) => {
            let mut bytes = Vec::with_capacity(8);
            let v = v.unwrap_or(i64::MIN);
            bytes.extend_from_slice(&(v ^ (1i64 << 63)).to_be_bytes());
            bytes
        }

        // Dates: store as i32 (days since epoch)
        Date32(v) => {
            let mut bytes = Vec::with_capacity(4);
            let v = v.unwrap_or(i32::MIN);
            bytes.extend_from_slice(&(v ^ (1 << 31)).to_be_bytes());
            bytes
        }
        Date64(v) => {
            let mut bytes = Vec::with_capacity(8);
            let v = v.unwrap_or(i64::MIN);
            bytes.extend_from_slice(&(v ^ (1i64 << 63)).to_be_bytes());
            bytes
        }

        // NULL values: return empty vector (will be handled separately)
        Null => Vec::new(),

        // Other types: not supported for clustering
        _ => Vec::new(),
    }
}

/// Get the appropriate clustering algorithm for the given number of dimensions.
pub fn get_algorithm(dimension: usize, _algorithm_name: Option<&str>) -> Result<Arc<dyn ClusteringAlgorithm>> {
    match dimension {
        1 => Ok(Arc::new(DirectSortAlgorithm::new())),
        2..=4 => {
            // For Phase 0, we only support 1D clustering
            // Hilbert will be implemented in Phase 3
            Err(Error::NotSupported {
                message: "Multi-dimensional clustering (Hilbert curve) not yet implemented in Phase 0".to_string(),
            })
        }
        _ => Err(Error::InvalidInput {
            message: format!("Invalid clustering dimension: {}", dimension),
        }),
    }
}

use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_sort_algorithm() {
        let algo = DirectSortAlgorithm::new();
        assert_eq!(algo.name(), "direct");

        // Test with a single value
        let value = ScalarValue::Int32(Some(42));
        let key = algo.compute_sort_key(&[value]).unwrap();
        assert!(!key.is_empty());

        // Test with wrong number of values
        let result = algo.compute_sort_key(&[
            ScalarValue::Int32(Some(1)),
            ScalarValue::Int32(Some(2)),
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_scalar_value_to_sort_key_ordering() {
        // Test i32 ordering
        let key_neg = scalar_value_to_sort_key(&ScalarValue::Int32(Some(-100)));
        let key_zero = scalar_value_to_sort_key(&ScalarValue::Int32(Some(0)));
        let key_pos = scalar_value_to_sort_key(&ScalarValue::Int32(Some(100)));
        assert!(key_neg < key_zero);
        assert!(key_zero < key_pos);

        // Test f64 ordering
        let key_neg_f = scalar_value_to_sort_key(&ScalarValue::Float64(Some(-100.0)));
        let key_zero_f = scalar_value_to_sort_key(&ScalarValue::Float64(Some(0.0)));
        let key_pos_f = scalar_value_to_sort_key(&ScalarValue::Float64(Some(100.0)));
        assert!(key_neg_f < key_zero_f);
        assert!(key_zero_f < key_pos_f);
    }

    #[test]
    fn test_get_algorithm() {
        // 1D should work
        let algo = get_algorithm(1, None);
        assert!(algo.is_ok());
        assert_eq!(algo.unwrap().name(), "direct");

        // 2D+ should fail in Phase 0
        let result = get_algorithm(2, None);
        match result {
            Err(e) => assert!(e.to_string().contains("not yet implemented")),
            Ok(_) => panic!("Expected error for 2D clustering"),
        }
    }
}
