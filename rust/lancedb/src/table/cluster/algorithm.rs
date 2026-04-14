// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: Copyright The LanceDB Authors

//! Clustering algorithms implementation.

use arrow::array::Array;
use datafusion::scalar::ScalarValue;
use hilbert_index::ToHilbertIndex;
use std::sync::RwLock;

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
                message: format!("Direct sort requires exactly 1 value, got {}", values.len()),
            });
        }
        Ok(scalar_value_to_sort_key(&values[0]))
    }
}

/// Hilbert curve algorithm for multi-dimensional clustering (2-4D).
///
/// Maps multi-dimensional coordinates to a 1D Hilbert index while
/// preserving spatial locality.
pub struct HilbertCurveAlgorithm {
    dimension: usize,
    bits: usize,
    bounds: RwLock<Vec<(f64, f64)>>,
}

impl HilbertCurveAlgorithm {
    /// Create a new Hilbert curve algorithm for the given dimension.
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            bits: 16,
            bounds: RwLock::new(Vec::with_capacity(dimension)),
        }
    }

    /// Set the number of bits used for coordinate quantization.
    pub fn with_bits(mut self, bits: usize) -> Self {
        self.bits = bits;
        self
    }
}

impl ClusteringAlgorithm for HilbertCurveAlgorithm {
    fn name(&self) -> &str {
        "hilbert"
    }

    fn prepare(&self, batches: &[arrow_array::RecordBatch], keys: &[String]) -> Result<()> {
        let mut mins = vec![f64::INFINITY; self.dimension];
        let mut maxs = vec![f64::NEG_INFINITY; self.dimension];

        for batch in batches {
            for (dim, key) in keys.iter().enumerate() {
                let col_idx = batch
                    .schema()
                    .index_of(key)
                    .map_err(|_| Error::InvalidInput {
                        message: format!("Column '{}' not found in batch", key),
                    })?;
                let array = batch.column(col_idx);
                for row in 0..array.len() {
                    if let Some(v) = array_element_to_f64(array.as_ref(), row) {
                        mins[dim] = mins[dim].min(v);
                        maxs[dim] = maxs[dim].max(v);
                    }
                }
            }
        }

        let bounds: Vec<(f64, f64)> = mins.into_iter().zip(maxs).collect();
        let mut lock = self.bounds.write().unwrap();
        *lock = bounds;
        Ok(())
    }

    fn compute_sort_key(&self, values: &[ScalarValue]) -> Result<Vec<u8>> {
        if values.len() != self.dimension {
            return Err(Error::InvalidInput {
                message: format!(
                    "Hilbert curve requires exactly {} values, got {}",
                    self.dimension,
                    values.len()
                ),
            });
        }

        let bounds = self.bounds.read().unwrap();
        if bounds.len() != self.dimension {
            return Err(Error::Runtime {
                message: "Hilbert algorithm not prepared".to_string(),
            });
        }

        let max_coord = (1usize << self.bits) - 1;
        let mut coords = vec![0usize; self.dimension];

        for (i, value) in values.iter().enumerate() {
            let Some(v) = scalar_value_to_f64(value) else {
                // NULL values are placed at the end
                return Ok(Vec::new());
            };
            let (min, max) = bounds[i];
            let normalized = if max == min {
                0.0
            } else {
                ((v - min) / (max - min)).clamp(0.0, 1.0)
            };
            coords[i] = (normalized * max_coord as f64).round() as usize;
        }

        let index = match self.dimension {
            2 => {
                let arr = [coords[0], coords[1]];
                arr.to_hilbert_index(self.bits)
            }
            3 => {
                let arr = [coords[0], coords[1], coords[2]];
                arr.to_hilbert_index(self.bits)
            }
            4 => {
                let arr = [coords[0], coords[1], coords[2], coords[3]];
                arr.to_hilbert_index(self.bits)
            }
            _ => {
                return Err(Error::InvalidInput {
                    message: format!(
                        "Hilbert curve does not support dimension {}",
                        self.dimension
                    ),
                });
            }
        };

        Ok(index.to_be_bytes().to_vec())
    }
}

/// Convert a ScalarValue to an optional f64 for normalization.
fn scalar_value_to_f64(value: &ScalarValue) -> Option<f64> {
    use ScalarValue::*;
    match value {
        Int8(v) => v.map(|x| x as f64),
        Int16(v) => v.map(|x| x as f64),
        Int32(v) => v.map(|x| x as f64),
        Int64(v) => v.map(|x| x as f64),
        UInt8(v) => v.map(|x| x as f64),
        UInt16(v) => v.map(|x| x as f64),
        UInt32(v) => v.map(|x| x as f64),
        UInt64(v) => v.map(|x| x as f64),
        Float32(v) => v.map(|x| x as f64),
        Float64(v) => *v,
        TimestampSecond(v, _) => v.map(|x| x as f64),
        TimestampMillisecond(v, _) => v.map(|x| x as f64),
        TimestampMicrosecond(v, _) => v.map(|x| x as f64),
        TimestampNanosecond(v, _) => v.map(|x| x as f64),
        Date32(v) => v.map(|x| x as f64),
        Date64(v) => v.map(|x| x as f64),
        Null => None,
        _ => None,
    }
}

/// Convert an array element at the given index to an optional f64.
fn array_element_to_f64(array: &dyn Array, index: usize) -> Option<f64> {
    use arrow::array::*;
    use arrow_schema::DataType;

    if !array.is_valid(index) {
        return None;
    }

    match array.data_type() {
        DataType::Int8 => Some(
            array
                .as_any()
                .downcast_ref::<Int8Array>()
                .unwrap()
                .value(index) as f64,
        ),
        DataType::Int16 => Some(
            array
                .as_any()
                .downcast_ref::<Int16Array>()
                .unwrap()
                .value(index) as f64,
        ),
        DataType::Int32 => Some(
            array
                .as_any()
                .downcast_ref::<Int32Array>()
                .unwrap()
                .value(index) as f64,
        ),
        DataType::Int64 => Some(
            array
                .as_any()
                .downcast_ref::<Int64Array>()
                .unwrap()
                .value(index) as f64,
        ),
        DataType::UInt8 => Some(
            array
                .as_any()
                .downcast_ref::<UInt8Array>()
                .unwrap()
                .value(index) as f64,
        ),
        DataType::UInt16 => Some(
            array
                .as_any()
                .downcast_ref::<UInt16Array>()
                .unwrap()
                .value(index) as f64,
        ),
        DataType::UInt32 => Some(
            array
                .as_any()
                .downcast_ref::<UInt32Array>()
                .unwrap()
                .value(index) as f64,
        ),
        DataType::UInt64 => Some(
            array
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap()
                .value(index) as f64,
        ),
        DataType::Float32 => Some(
            array
                .as_any()
                .downcast_ref::<Float32Array>()
                .unwrap()
                .value(index) as f64,
        ),
        DataType::Float64 => Some(
            array
                .as_any()
                .downcast_ref::<Float64Array>()
                .unwrap()
                .value(index),
        ),
        DataType::Timestamp(_, _) => {
            // Try all timestamp precisions
            if let Some(arr) = array.as_any().downcast_ref::<TimestampSecondArray>() {
                Some(arr.value(index) as f64)
            } else if let Some(arr) = array.as_any().downcast_ref::<TimestampMillisecondArray>() {
                Some(arr.value(index) as f64)
            } else if let Some(arr) = array.as_any().downcast_ref::<TimestampMicrosecondArray>() {
                Some(arr.value(index) as f64)
            } else if let Some(arr) = array.as_any().downcast_ref::<TimestampNanosecondArray>() {
                Some(arr.value(index) as f64)
            } else {
                None
            }
        }
        DataType::Date32 => Some(
            array
                .as_any()
                .downcast_ref::<Date32Array>()
                .unwrap()
                .value(index) as f64,
        ),
        DataType::Date64 => Some(
            array
                .as_any()
                .downcast_ref::<Date64Array>()
                .unwrap()
                .value(index) as f64,
        ),
        _ => None,
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
pub fn get_algorithm(
    dimension: usize,
    algorithm_name: Option<&str>,
    algorithm_params: Option<&serde_json::Value>,
) -> Result<std::sync::Arc<dyn ClusteringAlgorithm>> {
    let name = algorithm_name.unwrap_or("auto");
    match dimension {
        1 => {
            if name == "auto" || name == "direct" {
                Ok(std::sync::Arc::new(DirectSortAlgorithm::new()))
            } else {
                Err(Error::InvalidInput {
                    message: format!("Unknown clustering algorithm '{}' for 1D", name),
                })
            }
        }
        2..=4 => {
            if name == "auto" || name == "hilbert" {
                let bits = algorithm_params
                    .and_then(|p| p.get("bits"))
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(16);
                if dimension * bits > 64 {
                    return Err(Error::InvalidInput {
                        message: format!(
                            "Hilbert curve dimension ({}) * bits ({}) exceeds 64, which is not supported",
                            dimension, bits
                        ),
                    });
                }
                Ok(std::sync::Arc::new(
                    HilbertCurveAlgorithm::new(dimension).with_bits(bits),
                ))
            } else {
                Err(Error::InvalidInput {
                    message: format!("Unknown clustering algorithm '{}' for {}D", name, dimension),
                })
            }
        }
        _ => Err(Error::InvalidInput {
            message: format!("Invalid clustering dimension: {}", dimension),
        }),
    }
}

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
        let result =
            algo.compute_sort_key(&[ScalarValue::Int32(Some(1)), ScalarValue::Int32(Some(2))]);
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
        // 1D should return direct
        let algo = get_algorithm(1, None, None).unwrap();
        assert_eq!(algo.name(), "direct");

        // 2D should return hilbert
        let algo = get_algorithm(2, None, None).unwrap();
        assert_eq!(algo.name(), "hilbert");

        // 3D should return hilbert
        let algo = get_algorithm(3, None, None).unwrap();
        assert_eq!(algo.name(), "hilbert");

        // 4D should return hilbert
        let algo = get_algorithm(4, None, None).unwrap();
        assert_eq!(algo.name(), "hilbert");

        // Invalid dimension
        assert!(get_algorithm(5, None, None).is_err());
    }

    #[test]
    fn test_hilbert_curve_algorithm_2d() {
        let algo = HilbertCurveAlgorithm::new(2).with_bits(1);
        let schema = arrow_schema::Schema::new(vec![
            arrow_schema::Field::new("x", arrow_schema::DataType::Int64, false),
            arrow_schema::Field::new("y", arrow_schema::DataType::Int64, false),
        ]);
        let batch = arrow_array::RecordBatch::try_new(
            std::sync::Arc::new(schema),
            vec![
                std::sync::Arc::new(arrow_array::Int64Array::from(vec![0, 0, 1, 1])),
                std::sync::Arc::new(arrow_array::Int64Array::from(vec![0, 1, 1, 0])),
            ],
        )
        .unwrap();

        algo.prepare(&[batch], &["x".to_string(), "y".to_string()])
            .unwrap();

        let key_00 = algo
            .compute_sort_key(&[ScalarValue::Int64(Some(0)), ScalarValue::Int64(Some(0))])
            .unwrap();
        let key_01 = algo
            .compute_sort_key(&[ScalarValue::Int64(Some(0)), ScalarValue::Int64(Some(1))])
            .unwrap();
        let key_11 = algo
            .compute_sort_key(&[ScalarValue::Int64(Some(1)), ScalarValue::Int64(Some(1))])
            .unwrap();
        let key_10 = algo
            .compute_sort_key(&[ScalarValue::Int64(Some(1)), ScalarValue::Int64(Some(0))])
            .unwrap();

        assert!(key_00 < key_01);
        assert!(key_01 < key_11);
        assert!(key_11 < key_10);
    }

    #[test]
    fn test_hilbert_curve_algorithm_3d() {
        let algo = HilbertCurveAlgorithm::new(3).with_bits(1);
        let schema = arrow_schema::Schema::new(vec![
            arrow_schema::Field::new("x", arrow_schema::DataType::Int64, false),
            arrow_schema::Field::new("y", arrow_schema::DataType::Int64, false),
            arrow_schema::Field::new("z", arrow_schema::DataType::Int64, false),
        ]);
        let batch = arrow_array::RecordBatch::try_new(
            std::sync::Arc::new(schema),
            vec![
                std::sync::Arc::new(arrow_array::Int64Array::from(vec![0, 0, 0, 0, 1, 1, 1, 1])),
                std::sync::Arc::new(arrow_array::Int64Array::from(vec![0, 0, 1, 1, 0, 0, 1, 1])),
                std::sync::Arc::new(arrow_array::Int64Array::from(vec![0, 1, 0, 1, 0, 1, 0, 1])),
            ],
        )
        .unwrap();

        algo.prepare(
            &[batch],
            &["x".to_string(), "y".to_string(), "z".to_string()],
        )
        .unwrap();

        // Just verify all 8 points produce valid, distinct keys
        let mut keys = Vec::new();
        for x in [0, 1] {
            for y in [0, 1] {
                for z in [0, 1] {
                    let key = algo
                        .compute_sort_key(&[
                            ScalarValue::Int64(Some(x)),
                            ScalarValue::Int64(Some(y)),
                            ScalarValue::Int64(Some(z)),
                        ])
                        .unwrap();
                    keys.push(key);
                }
            }
        }
        keys.sort();
        // All 8 keys should be distinct
        for i in 1..keys.len() {
            assert_ne!(keys[i - 1], keys[i], "Hilbert keys should be distinct");
        }
    }

    #[test]
    fn test_hilbert_sort_key_locality() {
        // Points that are close in 2D space should have close Hilbert indices
        let algo = HilbertCurveAlgorithm::new(2).with_bits(8);
        let schema = arrow_schema::Schema::new(vec![
            arrow_schema::Field::new("x", arrow_schema::DataType::Int64, false),
            arrow_schema::Field::new("y", arrow_schema::DataType::Int64, false),
        ]);
        let batch = arrow_array::RecordBatch::try_new(
            std::sync::Arc::new(schema),
            vec![
                std::sync::Arc::new(arrow_array::Int64Array::from(vec![0, 100])),
                std::sync::Arc::new(arrow_array::Int64Array::from(vec![0, 100])),
            ],
        )
        .unwrap();

        algo.prepare(&[batch], &["x".to_string(), "y".to_string()])
            .unwrap();

        let key_near = algo
            .compute_sort_key(&[ScalarValue::Int64(Some(0)), ScalarValue::Int64(Some(0))])
            .unwrap();
        let key_far = algo
            .compute_sort_key(&[ScalarValue::Int64(Some(100)), ScalarValue::Int64(Some(100))])
            .unwrap();

        // Both keys should be valid (non-empty) and different
        assert!(!key_near.is_empty());
        assert!(!key_far.is_empty());
        assert_ne!(key_near, key_far);
    }

    #[test]
    fn test_hilbert_algorithm_with_null_values() {
        let algo = HilbertCurveAlgorithm::new(2).with_bits(1);
        let schema = arrow_schema::Schema::new(vec![
            arrow_schema::Field::new("x", arrow_schema::DataType::Int64, true),
            arrow_schema::Field::new("y", arrow_schema::DataType::Int64, true),
        ]);
        let batch = arrow_array::RecordBatch::try_new(
            std::sync::Arc::new(schema),
            vec![
                std::sync::Arc::new(arrow_array::Int64Array::from(vec![Some(0), Some(1)])),
                std::sync::Arc::new(arrow_array::Int64Array::from(vec![Some(0), None])),
            ],
        )
        .unwrap();

        algo.prepare(&[batch], &["x".to_string(), "y".to_string()])
            .unwrap();

        let key_valid = algo
            .compute_sort_key(&[ScalarValue::Int64(Some(0)), ScalarValue::Int64(Some(0))])
            .unwrap();
        let key_null = algo
            .compute_sort_key(&[ScalarValue::Int64(Some(1)), ScalarValue::Null])
            .unwrap();

        // NULL values should produce empty sort key
        assert!(key_null.is_empty());
        assert!(!key_valid.is_empty());
    }
}
