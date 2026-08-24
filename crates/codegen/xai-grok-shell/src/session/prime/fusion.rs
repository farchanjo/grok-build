//! Weighted reciprocal-rank fusion for skill Prime and `/skills` Smart search.
//!
//! Four independent rank lists are fused. Reranker scores are **not** a fifth
//! list and are never mixed with sqlite-vec L2 similarity:
//!
//! ```text
//! score(d) = Σ_s w_s / (k + rank_s(d))
//! k = 60
//! w_local  = 1.0   // when-to-use + inventory name evidence
//! w_path   = 1.0   // prompt-path glob evidence
//! w_bm25   = 0.8   // FTS/BM25 over the full indexed inventory
//! w_vector = 1.2   // sqlite-vec KNN over the full indexed inventory
//! ```
//!
//! Ranks are 1-indexed. A list that does not mention `d` contributes 0.
//! Vector similarity is `1 - L2/2` on unit-normalized embeddings (0..=1) and
//! is used only for the consumer/profile threshold, never as a rerank score.

use std::collections::{HashMap, HashSet};

/// Reciprocal-rank smoothing constant.
pub const RRF_K: f64 = 60.0;
/// Weight for deterministic local (when-to-use + inventory) rank.
pub const W_LOCAL: f64 = 1.0;
/// Weight for prompt-path glob rank.
pub const W_PATH: f64 = 1.0;
/// Weight for FTS/BM25 rank.
pub const W_BM25: f64 = 0.8;
/// Weight for vector KNN rank.
pub const W_VECTOR: f64 = 1.2;

/// Maximum L2 distance between two unit-norm embeddings.
const MAX_L2_DISTANCE: f32 = 2.0;

/// Convert sqlite-vec L2 distance on unit vectors to cosine-like similarity.
///
/// This is **not** a reranker score. Callers must keep the two domains
/// separate.
pub fn l2_similarity(distance: f32) -> f32 {
    if !distance.is_finite() {
        return 0.0;
    }
    (1.0 - (distance / MAX_L2_DISTANCE)).clamp(0.0, 1.0)
}

/// Reciprocal-rank contribution of a 1-indexed rank.
pub fn rrf_contrib(rank_1indexed: u32, weight: f64) -> f64 {
    if rank_1indexed == 0 {
        return 0.0;
    }
    weight / (RRF_K + f64::from(rank_1indexed))
}

/// One fused candidate. `idx` is the caller-side inventory index.
#[derive(Debug, Clone, PartialEq)]
pub struct FusedCandidate {
    pub idx: usize,
    pub local_score: i64,
    pub path_score: i64,
    pub bm25_rank: Option<u32>,
    pub vector_rank: Option<u32>,
    pub vector_similarity: Option<f32>,
    pub fused: f64,
}

/// Fuse local / path / BM25 / vector ranks.
///
/// `local_order` and `path_order` are best-first index lists (only candidates
/// with positive evidence). `bm25_order` / `vector_order` are opaque-id lists
/// in rank order. `id_to_idx` maps those ids onto `idx`.
pub fn fuse_ranks(
    candidate_count: usize,
    local_order: &[usize],
    path_order: &[usize],
    local_scores: &[i64],
    path_scores: &[i64],
    bm25_order: &[String],
    vector_order: &[String],
    vector_similarity: &HashMap<String, f32>,
    id_to_idx: &HashMap<String, usize>,
) -> Vec<FusedCandidate> {
    let mut out: Vec<FusedCandidate> = (0..candidate_count)
        .map(|idx| FusedCandidate {
            idx,
            local_score: local_scores.get(idx).copied().unwrap_or(0),
            path_score: path_scores.get(idx).copied().unwrap_or(0),
            bm25_rank: None,
            vector_rank: None,
            vector_similarity: None,
            fused: 0.0,
        })
        .collect();

    for (rank, &idx) in local_order.iter().enumerate() {
        if let Some(row) = out.get_mut(idx) {
            row.fused += rrf_contrib((rank as u32).saturating_add(1), W_LOCAL);
        }
    }
    for (rank, &idx) in path_order.iter().enumerate() {
        if let Some(row) = out.get_mut(idx) {
            row.fused += rrf_contrib((rank as u32).saturating_add(1), W_PATH);
        }
    }
    for (rank, id) in bm25_order.iter().enumerate() {
        if let Some(&idx) = id_to_idx.get(id)
            && let Some(row) = out.get_mut(idx)
        {
            let r = (rank as u32).saturating_add(1);
            row.bm25_rank = Some(r);
            row.fused += rrf_contrib(r, W_BM25);
        }
    }
    for (rank, id) in vector_order.iter().enumerate() {
        if let Some(&idx) = id_to_idx.get(id)
            && let Some(row) = out.get_mut(idx)
        {
            let r = (rank as u32).saturating_add(1);
            row.vector_rank = Some(r);
            row.vector_similarity = vector_similarity.get(id).copied();
            row.fused += rrf_contrib(r, W_VECTOR);
        }
    }

    out.sort_by(|a, b| {
        b.fused
            .total_cmp(&a.fused)
            .then_with(|| b.local_score.cmp(&a.local_score))
            .then_with(|| a.idx.cmp(&b.idx))
    });
    out
}

/// Whether an automatic (non-pinned) candidate may be selected.
///
/// Requires positive local/path evidence **or** vector similarity at/above
/// the stricter consumer/profile threshold. Similarity `0` is not evidence.
/// Missing KNN (`None`) is never vector evidence. FTS/BM25 rank is **not**
/// an admission signal: [`fuse_ranks`] may still use it to reorder rows that
/// already qualified.
pub fn automatic_candidate_allowed(
    local_score: i64,
    path_score: i64,
    vector_similarity: Option<f32>,
    threshold: f32,
) -> bool {
    if local_score > 0 || path_score > 0 {
        return true;
    }
    match vector_similarity {
        Some(sim) if sim.is_finite() => {
            if threshold <= 0.0 {
                sim > 0.0
            } else {
                sim >= threshold
            }
        }
        _ => false,
    }
}

/// Stricter of the consumer and profile floors, clamped to `[0, 1]`.
pub fn stricter_threshold(consumer: f32, profile: f32) -> f32 {
    consumer.max(profile).clamp(0.0, 1.0)
}

/// Stable unique order: `preferred` first, then `rest`, no duplicates.
pub fn prepend_unique(preferred: &[usize], rest: &[usize]) -> Vec<usize> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(preferred.len() + rest.len());
    for &i in preferred.iter().chain(rest.iter()) {
        if seen.insert(i) {
            out.push(i);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrf_weights_are_documented() {
        assert_eq!(RRF_K, 60.0);
        assert_eq!(W_LOCAL, 1.0);
        assert_eq!(W_PATH, 1.0);
        assert_eq!(W_BM25, 0.8);
        assert_eq!(W_VECTOR, 1.2);
        let a = rrf_contrib(1, W_VECTOR);
        let b = rrf_contrib(2, W_VECTOR);
        assert!(a > b);
        assert_eq!(rrf_contrib(0, W_VECTOR), 0.0);
    }

    #[test]
    fn l2_similarity_is_not_a_rerank_score() {
        assert!((l2_similarity(0.0) - 1.0).abs() < f32::EPSILON);
        assert!((l2_similarity(2.0) - 0.0).abs() < f32::EPSILON);
        assert_eq!(l2_similarity(f32::NAN), 0.0);
    }

    #[test]
    fn fuse_prefers_higher_vector_rank_when_local_ties() {
        let local: Vec<usize> = Vec::new();
        let path: Vec<usize> = Vec::new();
        let local_scores = vec![0, 0, 0];
        let path_scores = vec![0, 0, 0];
        let bm25: Vec<String> = Vec::new();
        let vector = vec!["c".into(), "a".into()];
        let mut sim = HashMap::new();
        sim.insert("c".into(), 0.9);
        sim.insert("a".into(), 0.5);
        let mut id_to_idx = HashMap::new();
        id_to_idx.insert("a".into(), 0);
        id_to_idx.insert("b".into(), 1);
        id_to_idx.insert("c".into(), 2);
        let fused = fuse_ranks(
            3,
            &local,
            &path,
            &local_scores,
            &path_scores,
            &bm25,
            &vector,
            &sim,
            &id_to_idx,
        );
        assert_eq!(
            fused[0].idx, 2,
            "strongest vector rank should win a local tie"
        );
        assert_eq!(fused[0].vector_similarity, Some(0.9));
        assert!(fused[0].fused > fused[1].fused);
    }

    #[test]
    fn automatic_candidate_requires_local_or_threshold() {
        assert!(automatic_candidate_allowed(4, 0, None, 0.4));
        assert!(automatic_candidate_allowed(0, 2, None, 0.4));
        assert!(!automatic_candidate_allowed(0, 0, None, 0.4));
        assert!(!automatic_candidate_allowed(0, 0, Some(0.2), 0.4));
        assert!(automatic_candidate_allowed(0, 0, Some(0.4), 0.4));
        assert!(automatic_candidate_allowed(0, 0, Some(0.01), 0.0));
        assert!(!automatic_candidate_allowed(0, 0, Some(0.0), 0.0));
        assert!(
            !automatic_candidate_allowed(0, 0, None, 0.0),
            "missing KNN is not vector evidence even at threshold 0"
        );
    }

    #[test]
    fn bm25_rank_reorders_without_admitting() {
        let local: Vec<usize> = Vec::new();
        let path: Vec<usize> = Vec::new();
        let local_scores = vec![0, 0];
        let path_scores = vec![0, 0];
        let bm25 = vec!["b".into()];
        let vector: Vec<String> = Vec::new();
        let sim = HashMap::new();
        let mut id_to_idx = HashMap::new();
        id_to_idx.insert("a".into(), 0);
        id_to_idx.insert("b".into(), 1);
        let fused = fuse_ranks(
            2,
            &local,
            &path,
            &local_scores,
            &path_scores,
            &bm25,
            &vector,
            &sim,
            &id_to_idx,
        );
        assert_eq!(fused[0].idx, 1, "BM25 must still rank an FTS-only row");
        assert_eq!(fused[0].bm25_rank, Some(1));
        assert!(
            !automatic_candidate_allowed(
                fused[0].local_score,
                fused[0].path_score,
                fused[0].vector_similarity,
                0.4
            ),
            "an FTS/BM25 hit without local/path/vector evidence must not be admitted"
        );
    }

    #[test]
    fn stricter_threshold_takes_max() {
        assert!((stricter_threshold(0.2, 0.4) - 0.4).abs() < f32::EPSILON);
        assert!((stricter_threshold(0.5, 0.1) - 0.5).abs() < f32::EPSILON);
        assert_eq!(stricter_threshold(-1.0, 2.0), 1.0);
    }

    #[test]
    fn prepend_unique_keeps_pins_first() {
        assert_eq!(prepend_unique(&[2], &[0, 2, 1]), vec![2, 0, 1]);
    }
}
