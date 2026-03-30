//! Embedding vector type with similarity operations.

use serde::{Deserialize, Serialize};

use crate::simd;

/// A single embedding vector with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// The embedding vector (normalized to unit length).
    pub vector: Vec<f32>,

    /// Hash of the original text for cache lookups.
    pub text_hash: u64,
}

impl Embedding {
    /// Create a new embedding from a vector.
    /// Automatically normalizes the vector to unit length.
    pub fn new(mut vector: Vec<f32>, text_hash: u64) -> Self {
        simd::normalize(&mut vector);
        Self { vector, text_hash }
    }

    /// Create an embedding without normalizing (assumes already normalized).
    pub fn from_normalized(vector: Vec<f32>, text_hash: u64) -> Self {
        Self { vector, text_hash }
    }

    /// Get the dimensionality of this embedding.
    pub fn dimensions(&self) -> usize {
        self.vector.len()
    }

    /// Cosine similarity with another embedding.
    /// For normalized vectors, this equals the dot product.
    pub fn cosine_similarity(&self, other: &Embedding) -> f32 {
        // For pre-normalized vectors, cosine similarity = dot product
        simd::dot_product(&self.vector, &other.vector)
    }

    /// Dot product (for pre-normalized vectors, equals cosine similarity).
    pub fn dot_product(&self, other: &Embedding) -> f32 {
        simd::dot_product(&self.vector, &other.vector)
    }

    /// Euclidean distance to another embedding.
    pub fn euclidean_distance(&self, other: &Embedding) -> f32 {
        let mut sum = 0.0f32;
        for (a, b) in self.vector.iter().zip(other.vector.iter()) {
            let diff = a - b;
            sum += diff * diff;
        }
        sum.sqrt()
    }

    /// L2 norm of the vector.
    pub fn norm(&self) -> f32 {
        simd::dot_product(&self.vector, &self.vector).sqrt()
    }

    /// Check if this embedding is normalized (unit length).
    pub fn is_normalized(&self) -> bool {
        let norm = self.norm();
        (norm - 1.0).abs() < 1e-5
    }
}

impl PartialEq for Embedding {
    fn eq(&self, other: &Self) -> bool {
        self.text_hash == other.text_hash && self.vector == other.vector
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_embedding(values: &[f32]) -> Embedding {
        Embedding::new(values.to_vec(), 0)
    }

    #[test]
    fn test_embedding_normalization() {
        let embed = make_embedding(&[3.0, 4.0]);
        assert!(embed.is_normalized());
        // 3-4-5 triangle, normalized: (0.6, 0.8)
        assert!((embed.vector[0] - 0.6).abs() < 1e-5);
        assert!((embed.vector[1] - 0.8).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let embed = make_embedding(&[1.0, 0.0, 0.0]);
        let similarity = embed.cosine_similarity(&embed);
        assert!((similarity - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = make_embedding(&[1.0, 0.0]);
        let b = make_embedding(&[0.0, 1.0]);
        let similarity = a.cosine_similarity(&b);
        assert!(similarity.abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = make_embedding(&[1.0, 0.0]);
        let b = make_embedding(&[-1.0, 0.0]);
        let similarity = a.cosine_similarity(&b);
        assert!((similarity + 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_euclidean_distance() {
        let a = make_embedding(&[1.0, 0.0]);
        let b = make_embedding(&[0.0, 1.0]);
        let dist = a.euclidean_distance(&b);
        // For normalized vectors, opposite corners: sqrt(2) * something
        assert!(dist > 0.0);
    }

    #[test]
    fn test_from_normalized() {
        let embed = Embedding::from_normalized(vec![0.6, 0.8], 123);
        assert!(embed.is_normalized());
        assert_eq!(embed.text_hash, 123);
    }

    #[test]
    fn test_dimensions() {
        let embed = make_embedding(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(embed.dimensions(), 4);
    }
}
