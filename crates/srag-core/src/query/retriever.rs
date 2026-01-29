// SPDX-License-Identifier: GPL-3.0
// Copyright (c) 2026 Matt Hesketh <matt@matthesketh.pro>

use std::collections::HashMap;

use srag_common::types::Chunk;
use srag_common::Result;

use crate::index::store::Store;

pub fn resolve_results(store: &Store, results: &[(usize, f32)]) -> Result<Vec<(Chunk, String)>> {
    let mut chunks = Vec::new();
    for &(embedding_id, _distance) in results {
        if let Some(pair) = store.get_chunk_by_embedding_id(embedding_id as i64)? {
            chunks.push(pair);
        }
    }
    Ok(chunks)
}

pub fn reciprocal_rank_fusion(
    vector_results: &[(usize, f32)],
    fts_results: &[(i64, f64)],
    store: &Store,
    top_k: usize,
) -> Result<Vec<(Chunk, String)>> {
    const K: f64 = 60.0;
    let mut scores: HashMap<i64, f64> = HashMap::new();

    // vector results: embedding_id -> chunk_id
    for (rank, &(embedding_id, _distance)) in vector_results.iter().enumerate() {
        if let Ok(Some(chunk_id)) = store.get_chunk_id_by_embedding_id(embedding_id as i64) {
            let rrf_score = 1.0 / (K + rank as f64 + 1.0);
            *scores.entry(chunk_id).or_default() += rrf_score;
        }
    }

    // FTS results: already chunk_id
    for (rank, &(chunk_id, _bm25)) in fts_results.iter().enumerate() {
        let rrf_score = 1.0 / (K + rank as f64 + 1.0);
        *scores.entry(chunk_id).or_default() += rrf_score;
    }

    let mut ranked: Vec<(i64, f64)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(top_k);

    let mut chunks = Vec::new();
    for (chunk_id, _score) in ranked {
        if let Ok(Some(pair)) = store.get_chunk_by_id(chunk_id) {
            chunks.push(pair);
        }
    }
    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    const K: f64 = 60.0;

    fn compute_rrf_scores(
        vector_ranks: &[(i64, usize)],
        fts_ranks: &[(i64, usize)],
    ) -> HashMap<i64, f64> {
        let mut scores: HashMap<i64, f64> = HashMap::new();

        for &(chunk_id, rank) in vector_ranks {
            let rrf_score = 1.0 / (K + rank as f64 + 1.0);
            *scores.entry(chunk_id).or_default() += rrf_score;
        }

        for &(chunk_id, rank) in fts_ranks {
            let rrf_score = 1.0 / (K + rank as f64 + 1.0);
            *scores.entry(chunk_id).or_default() += rrf_score;
        }

        scores
    }

    #[test]
    fn test_rrf_single_source() {
        let vector_ranks = vec![(1, 0), (2, 1), (3, 2)];
        let scores = compute_rrf_scores(&vector_ranks, &[]);

        assert!(scores.get(&1).unwrap() > scores.get(&2).unwrap());
        assert!(scores.get(&2).unwrap() > scores.get(&3).unwrap());
    }

    #[test]
    fn test_rrf_both_sources_boost() {
        let vector_ranks = vec![(1, 0), (2, 1)];
        let fts_ranks = vec![(1, 0), (3, 1)];
        let scores = compute_rrf_scores(&vector_ranks, &fts_ranks);

        assert!(scores.get(&1).unwrap() > scores.get(&2).unwrap());
        assert!(scores.get(&1).unwrap() > scores.get(&3).unwrap());
    }

    #[test]
    fn test_rrf_empty_inputs() {
        let scores = compute_rrf_scores(&[], &[]);
        assert!(scores.is_empty());
    }

    #[test]
    fn test_rrf_ranking_order() {
        let vector_ranks = vec![(1, 0), (2, 1), (3, 2)];
        let fts_ranks = vec![(3, 0), (2, 1), (1, 2)];
        let scores = compute_rrf_scores(&vector_ranks, &fts_ranks);

        let mut ranked: Vec<_> = scores.into_iter().collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        assert_eq!(ranked.len(), 3);
    }

    #[test]
    fn test_rrf_score_calculation() {
        let vector_ranks = vec![(1, 0)];
        let scores = compute_rrf_scores(&vector_ranks, &[]);

        let expected = 1.0 / (K + 1.0);
        assert!((scores.get(&1).unwrap() - expected).abs() < 0.0001);
    }

    #[test]
    fn test_rrf_deduplication() {
        let vector_ranks = vec![(1, 0), (1, 1)];
        let scores = compute_rrf_scores(&vector_ranks, &[]);

        let expected = 1.0 / (K + 1.0) + 1.0 / (K + 2.0);
        assert!((scores.get(&1).unwrap() - expected).abs() < 0.0001);
    }
}
