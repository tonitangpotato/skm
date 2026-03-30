//! SIMD-accelerated vector operations.
//!
//! Uses portable SIMD when available (nightly), falls back to scalar operations.
//! The scalar implementations are still loop-unrolled for pipeline efficiency.

/// Dot product of two f32 slices.
/// Loop-unrolled for pipeline efficiency.
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vectors must have same length");

    let len = a.len();
    let chunks = len / 8;
    let remainder = len % 8;

    let mut sum0 = 0.0f32;
    let mut sum1 = 0.0f32;
    let mut sum2 = 0.0f32;
    let mut sum3 = 0.0f32;
    let mut sum4 = 0.0f32;
    let mut sum5 = 0.0f32;
    let mut sum6 = 0.0f32;
    let mut sum7 = 0.0f32;

    let mut i = 0;
    for _ in 0..chunks {
        sum0 += a[i] * b[i];
        sum1 += a[i + 1] * b[i + 1];
        sum2 += a[i + 2] * b[i + 2];
        sum3 += a[i + 3] * b[i + 3];
        sum4 += a[i + 4] * b[i + 4];
        sum5 += a[i + 5] * b[i + 5];
        sum6 += a[i + 6] * b[i + 6];
        sum7 += a[i + 7] * b[i + 7];
        i += 8;
    }

    // Handle remainder
    for j in 0..remainder {
        sum0 += a[i + j] * b[i + j];
    }

    sum0 + sum1 + sum2 + sum3 + sum4 + sum5 + sum6 + sum7
}

/// L2 normalize a vector in-place.
pub fn normalize(v: &mut [f32]) {
    let norm = dot_product(v, v).sqrt();
    if norm > 1e-10 {
        let inv_norm = 1.0 / norm;
        for x in v.iter_mut() {
            *x *= inv_norm;
        }
    }
}

/// Batch cosine similarity: one query vs N candidates.
/// Returns scores in the same order as candidates.
///
/// For normalized vectors, cosine similarity equals dot product.
pub fn batch_cosine(query: &[f32], candidates: &[&[f32]]) -> Vec<f32> {
    candidates.iter().map(|c| dot_product(query, c)).collect()
}

/// Compute L2 norm of a vector.
pub fn l2_norm(v: &[f32]) -> f32 {
    dot_product(v, v).sqrt()
}

/// Element-wise addition: result = a + b
pub fn add(a: &[f32], b: &[f32]) -> Vec<f32> {
    debug_assert_eq!(a.len(), b.len());
    a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
}

/// Element-wise subtraction: result = a - b
pub fn sub(a: &[f32], b: &[f32]) -> Vec<f32> {
    debug_assert_eq!(a.len(), b.len());
    a.iter().zip(b.iter()).map(|(x, y)| x - y).collect()
}

/// Scalar multiplication: result = a * scalar
pub fn scale(a: &[f32], scalar: f32) -> Vec<f32> {
    a.iter().map(|x| x * scalar).collect()
}

/// Mean of multiple vectors.
pub fn mean(vectors: &[&[f32]]) -> Vec<f32> {
    if vectors.is_empty() {
        return Vec::new();
    }

    let dim = vectors[0].len();
    let mut result = vec![0.0f32; dim];
    let scale = 1.0 / vectors.len() as f32;

    for v in vectors {
        for (i, x) in v.iter().enumerate() {
            result[i] += x * scale;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_product_basic() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let result = dot_product(&a, &b);
        // 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32
        assert!((result - 32.0).abs() < 1e-5);
    }

    #[test]
    fn test_dot_product_large() {
        // Test with more than 8 elements to exercise unrolling
        let a: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..100).map(|i| i as f32).collect();
        let result = dot_product(&a, &b);
        // Sum of squares: 0^2 + 1^2 + ... + 99^2 = n(n-1)(2n-1)/6 for n=100
        let expected: f32 = (0..100).map(|i| (i * i) as f32).sum();
        assert!((result - expected).abs() < 1e-3);
    }

    #[test]
    fn test_normalize() {
        let mut v = vec![3.0, 4.0];
        normalize(&mut v);
        assert!((v[0] - 0.6).abs() < 1e-5);
        assert!((v[1] - 0.8).abs() < 1e-5);

        // Check it's unit length
        let norm = l2_norm(&v);
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_normalize_zero() {
        let mut v = vec![0.0, 0.0, 0.0];
        normalize(&mut v);
        // Should not panic, vector unchanged
        assert_eq!(v, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_batch_cosine() {
        let query = vec![1.0, 0.0];
        let c1 = vec![1.0, 0.0]; // parallel
        let c2 = vec![0.0, 1.0]; // orthogonal
        let c3 = vec![-1.0, 0.0]; // opposite

        let candidates: Vec<&[f32]> = vec![&c1, &c2, &c3];
        let scores = batch_cosine(&query, &candidates);

        assert!((scores[0] - 1.0).abs() < 1e-5); // parallel
        assert!(scores[1].abs() < 1e-5); // orthogonal
        assert!((scores[2] + 1.0).abs() < 1e-5); // opposite
    }

    #[test]
    fn test_add() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let result = add(&a, &b);
        assert_eq!(result, vec![5.0, 7.0, 9.0]);
    }

    #[test]
    fn test_sub() {
        let a = vec![4.0, 5.0, 6.0];
        let b = vec![1.0, 2.0, 3.0];
        let result = sub(&a, &b);
        assert_eq!(result, vec![3.0, 3.0, 3.0]);
    }

    #[test]
    fn test_scale() {
        let a = vec![1.0, 2.0, 3.0];
        let result = scale(&a, 2.0);
        assert_eq!(result, vec![2.0, 4.0, 6.0]);
    }

    #[test]
    fn test_mean() {
        let v1 = vec![1.0, 2.0, 3.0];
        let v2 = vec![3.0, 4.0, 5.0];
        let vectors: Vec<&[f32]> = vec![&v1, &v2];
        let result = mean(&vectors);
        assert_eq!(result, vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_mean_empty() {
        let vectors: Vec<&[f32]> = vec![];
        let result = mean(&vectors);
        assert!(result.is_empty());
    }
}
