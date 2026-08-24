//! Deterministic local-only and mock-vector routing quality coverage.

use std::collections::HashMap;

use xai_grok_memory::l2_normalize_v1;
use xai_grok_tools::implementations::skills::strict::{LocalSkillEvidence, text_leaks_secrets};
use xai_grok_tools::implementations::skills::types::{SkillInfo, SkillScope};

use super::fusion::{automatic_candidate_allowed, fuse_ranks, l2_similarity};
use super::index::skill_rerank_document;
use super::inventory::WorkspaceInventory;
use super::skills::{evidence_lists, rank_skills};

const K: usize = 3;

struct QualityCase {
    query: &'static str,
    relevant: &'static [&'static str],
}

fn corpus() -> Vec<SkillInfo> {
    vec![
        // when-to-use lists both the literal trigger and the git-history
        // paraphrase so local overlap (and the hermetic mock vector document)
        // can admit the skill without body text or a weakened threshold.
        skill(
            "commit",
            "commit changes; record work in git history",
            "Create well-formatted git commits.",
        ),
        skill(
            "review",
            "review a pull request",
            "Review a pull request for correctness.",
        ),
        skill(
            "deploy",
            "deploy the release",
            "Deploy the service release.",
        ),
        skill("format", "format rust sources", "Format rust sources."),
        skill("zzzz", "unrelated zebra", "Unrelated noise skill."),
    ]
}

fn skill(name: &str, when_to_use: &str, description: &str) -> SkillInfo {
    SkillInfo {
        name: name.into(),
        path: format!("skills/{name}/SKILL.md"),
        description: description.into(),
        has_user_specified_description: true,
        when_to_use: Some(when_to_use.into()),
        paths: Some(vec!["src/**".into()]),
        scope: SkillScope::Repo,
        enabled: true,
        body: Some("SECRET-BODY /Users/me/secret".into()),
        ..SkillInfo::default()
    }
}

fn cases() -> &'static [QualityCase] {
    &[
        QualityCase {
            query: "please commit changes now",
            relevant: &["commit"],
        },
        QualityCase {
            query: "please review a pull request",
            relevant: &["review"],
        },
        QualityCase {
            query: "please deploy the release",
            relevant: &["deploy"],
        },
        QualityCase {
            query: "please format rust sources",
            relevant: &["format"],
        },
        QualityCase {
            query: "what is the weather today",
            relevant: &[],
        },
        QualityCase {
            query: "record this work in git history",
            relevant: &["commit"],
        },
        QualityCase {
            query: "review the pull request not the release train",
            relevant: &["review"],
        },
    ]
}

fn mock_bm25_order(skills: &[SkillInfo], query: &str) -> Vec<String> {
    let q: Vec<String> = query
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_ascii_alphanumeric())
                .to_ascii_lowercase()
        })
        .filter(|t| t.len() >= 4)
        .collect();
    let mut scored: Vec<(String, i64)> = skills
        .iter()
        .map(|s| {
            let doc = format!("{} {}", s.name, s.description).to_ascii_lowercase();
            let score = q.iter().filter(|t| doc.contains(t.as_str())).count() as i64;
            (opaque(s), score)
        })
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    scored
        .into_iter()
        .filter(|(_, score)| *score > 0)
        .map(|(id, _)| id)
        .collect()
}

fn recall_at_k(ranked: &[String], relevant: &[&str], k: usize) -> f64 {
    if relevant.is_empty() {
        return 1.0;
    }
    let hit = ranked
        .iter()
        .take(k)
        .filter(|name| relevant.contains(&name.as_str()))
        .count();
    hit as f64 / relevant.len() as f64
}

fn precision_at_k(ranked: &[String], relevant: &[&str], k: usize) -> f64 {
    if k == 0 {
        return 1.0;
    }
    if relevant.is_empty() {
        return if ranked.iter().take(k).count() == 0 {
            1.0
        } else {
            0.0
        };
    }
    let hit = ranked
        .iter()
        .take(k)
        .filter(|name| relevant.contains(&name.as_str()))
        .count();
    hit as f64 / k.min(ranked.len()).max(1) as f64
}

fn false_positive_rate(ranked: &[String], relevant: &[&str], k: usize) -> f64 {
    let considered = ranked.iter().take(k).count();
    if considered == 0 {
        return 0.0;
    }
    let fp = ranked
        .iter()
        .take(k)
        .filter(|name| !relevant.contains(&name.as_str()))
        .count();
    fp as f64 / considered as f64
}

fn names_from(skills: &[SkillInfo], order: &[usize]) -> Vec<String> {
    order.iter().map(|&i| skills[i].name.clone()).collect()
}

/// Deterministic mock embedding: overlapping tokens share dimensions.
///
/// Tokens of length >= 3 are kept (except a small stop list) so domain
/// signals such as `git` in metadata can contribute hermetic evidence
/// without shipping bodies or absolute paths.
fn mock_embed(text: &str, dims: usize) -> Vec<f32> {
    let mut values = vec![0.0f32; dims];
    const STOP: &[&str] = &[
        "the", "and", "for", "please", "what", "is", "a", "to", "now", "today", "when", "this",
        "that", "with", "from", "into", "not", "but", "are", "was",
    ];
    for word in text.split_whitespace() {
        let token = word
            .trim_matches(|c: char| !c.is_ascii_alphanumeric())
            .to_ascii_lowercase();
        if token.len() < 3 || STOP.contains(&token.as_str()) {
            continue;
        }
        let mut hash = 2166136261u32;
        for b in token.bytes() {
            hash ^= u32::from(b);
            hash = hash.wrapping_mul(16777619);
        }
        let idx = hash as usize % dims;
        values[idx] += 1.0;
    }
    if values.iter().all(|v| *v == 0.0) {
        values[0] = 1.0;
    }
    l2_normalize_v1(&mut values).expect("mock embed normalizes");
    values
}

fn l2(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f32>()
        .sqrt()
}

fn mock_vector_order(skills: &[SkillInfo], query: &str) -> (Vec<String>, HashMap<String, f32>) {
    let q = mock_embed(query, 8);
    let mut scored: Vec<(String, f32, f32)> = skills
        .iter()
        .map(|s| {
            let doc = skill_rerank_document(s);
            let emb = mock_embed(&doc, 8);
            let dist = l2(&q, &emb);
            (opaque(s), dist, l2_similarity(dist))
        })
        .collect();
    scored.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
    let mut sim = HashMap::new();
    let mut order = Vec::new();
    for (id, _, similarity) in scored {
        sim.insert(id.clone(), similarity);
        order.push(id);
    }
    (order, sim)
}

fn opaque(skill: &SkillInfo) -> String {
    format!("{}:{}", skill.scope as u8, skill.name)
}

fn fused_order(
    skills: &[SkillInfo],
    query: &str,
    inventory: &WorkspaceInventory,
    with_vectors: bool,
) -> Vec<usize> {
    let (local_scores, path_scores, local_order, path_order) =
        evidence_lists(skills, query, inventory);
    let id_to_idx: HashMap<String, usize> = skills
        .iter()
        .enumerate()
        .map(|(i, s)| (opaque(s), i))
        .collect();
    let (vector_order, vector_sim) = if with_vectors {
        mock_vector_order(skills, query)
    } else {
        (Vec::new(), HashMap::new())
    };
    let bm25_order = mock_bm25_order(skills, query);
    let fused = fuse_ranks(
        skills.len(),
        &local_order,
        &path_order,
        &local_scores,
        &path_scores,
        &bm25_order,
        &vector_order,
        &vector_sim,
        &id_to_idx,
    );
    fused.into_iter().map(|row| row.idx).collect()
}

fn automatic_topk(
    skills: &[SkillInfo],
    query: &str,
    inventory: &WorkspaceInventory,
    order: &[usize],
    with_vectors: bool,
) -> Vec<String> {
    let (local_scores, path_scores, _, _) = evidence_lists(skills, query, inventory);
    let vector_sim = if with_vectors {
        mock_vector_order(skills, query).1
    } else {
        HashMap::new()
    };
    order
        .iter()
        .copied()
        .filter(|&idx| {
            automatic_candidate_allowed(
                local_scores[idx],
                path_scores[idx],
                vector_sim.get(&opaque(&skills[idx])).copied(),
                0.65,
            )
        })
        .map(|idx| skills[idx].name.clone())
        .take(K)
        .collect()
}

#[test]
fn routing_quality_corpus_is_metadata_only() {
    for skill in corpus() {
        let doc = skill_rerank_document(&skill);
        assert!(
            !doc.contains("SECRET-BODY"),
            "quality corpus must never index bodies"
        );
        assert!(
            !doc.contains("/Users/"),
            "quality corpus must never index absolute paths"
        );
        let evidence = LocalSkillEvidence::from_skill_info(&skill);
        assert!(evidence.paths.iter().all(|p| !p.contains("/Users/")));
        if let Some(token) = text_leaks_secrets(&doc) {
            panic!("index document leaked {token}");
        }
    }
}

#[test]
fn local_only_recall_precision_negatives_and_stability() {
    let skills = corpus();
    let inv = WorkspaceInventory::default();
    let mut recall = 0.0;
    let mut precision = 0.0;
    let mut fpr = 0.0;
    let n = cases().len() as f64;
    for case in cases() {
        let order = rank_skills(&skills, case.query, &inv, None);
        let ranked = names_from(&skills, &order);
        let auto = automatic_topk(&skills, case.query, &inv, &order, false);
        if case.relevant.is_empty() {
            assert!(
                auto.is_empty(),
                "negative trigger {} selected {auto:?}",
                case.query
            );
        } else {
            assert_eq!(
                auto.first().map(String::as_str),
                Some(case.relevant[0]),
                "local@1 for {}",
                case.query
            );
        }
        recall += recall_at_k(&auto, case.relevant, 1);
        precision += precision_at_k(&auto, case.relevant, 1);
        fpr += if case.relevant.is_empty() {
            false_positive_rate(&auto, case.relevant, K)
        } else {
            0.0
        };
        let again = rank_skills(&skills, case.query, &inv, None);
        assert_eq!(order, again, "local ranking must be stable");
        assert_eq!(ranked, names_from(&skills, &again));
    }
    assert!(
        recall / n >= 1.0,
        "local recall@1 must be 1.0 on this corpus, got {}",
        recall / n
    );
    assert!(
        precision / n >= 0.8,
        "local precision@1 too low: {}",
        precision / n
    );
    let negative_n = cases().iter().filter(|c| c.relevant.is_empty()).count() as f64;
    assert_eq!(fpr / negative_n.max(1.0), 0.0);
}

#[test]
fn mock_vector_recall_and_degradation_parity() {
    let skills = corpus();
    let inv = WorkspaceInventory::default();
    for case in cases() {
        let local_order = rank_skills(&skills, case.query, &inv, None);
        let local_auto = automatic_topk(&skills, case.query, &inv, &local_order, false);
        let fused = fused_order(&skills, case.query, &inv, true);
        let fused_auto = automatic_topk(&skills, case.query, &inv, &fused, true);
        let degraded = fused_order(&skills, case.query, &inv, false);
        let degraded_auto = automatic_topk(&skills, case.query, &inv, &degraded, false);
        assert_eq!(
            local_auto, degraded_auto,
            "vector-unavailable fusion must match local-only automatic set for {}",
            case.query
        );
        if case.relevant.is_empty() {
            assert!(
                fused_auto.is_empty(),
                "negative trigger {} selected {fused_auto:?}",
                case.query
            );
            continue;
        }
        assert!(
            recall_at_k(&fused_auto, case.relevant, K) >= 1.0,
            "mock-vector recall@k for {} was {:?}",
            case.query,
            fused_auto
        );
        let again = fused_order(&skills, case.query, &inv, true);
        assert_eq!(fused, again, "mock-vector fusion must be stable");
    }
}

#[test]
fn per_skill_local_regression_status_is_current_or_explicitly_stale() {
    use std::sync::atomic::AtomicBool;
    use xai_grok_tools::implementations::skills::strict::{
        EvalCase, EvalCaseKind, EvalSuite, SkillIdentity, run_eval_suite,
    };

    let skills = corpus();
    for skill in &skills {
        if skill.name == "zzzz" {
            continue;
        }
        let suite = EvalSuite {
            version: 1,
            cases: vec![
                EvalCase {
                    id: format!("should-{}", skill.name),
                    kind: EvalCaseKind::ShouldTrigger,
                    query: skill.when_to_use.clone().unwrap(),
                    skill: Some(skill.name.clone()),
                    path: None,
                    resource: None,
                    peers: Vec::new(),
                },
                EvalCase {
                    id: format!("not-{}", skill.name),
                    kind: EvalCaseKind::ShouldNotTrigger,
                    query: "weather".into(),
                    skill: Some(skill.name.clone()),
                    path: None,
                    resource: None,
                    peers: Vec::new(),
                },
            ],
        };
        let evidence = LocalSkillEvidence::from_skill_info(skill);
        let report = run_eval_suite(
            &suite,
            &evidence,
            &[],
            SkillIdentity::new(&skill.name, Some(skill.scope)),
            1,
            "quality",
            &AtomicBool::new(false),
        );
        assert!(
            report.stable,
            "{} local regression must be stable",
            skill.name
        );
        assert_eq!(
            report.status.as_str(),
            "valid-pass",
            "{} regression must be current valid-pass, got {}",
            skill.name,
            report.status.as_str()
        );
        let stale = report.is_stale("quality", "other");
        assert!(stale, "fingerprint mismatch must be reported stale");
    }
}

#[test]
fn nonempty_bm25_list_changes_fused_order_from_empty_bm25() {
    let skills = corpus();
    let zeros = vec![0i64; skills.len()];
    let id_to_idx: HashMap<String, usize> = skills
        .iter()
        .enumerate()
        .map(|(i, s)| (opaque(s), i))
        .collect();
    // Isolate BM25 as the fourth list: with no local/path/vector evidence,
    // empty BM25 ties break by idx, while a reverse BM25 order must invert.
    let reverse_bm25: Vec<String> = skills.iter().rev().map(opaque).collect();
    let empty = fuse_ranks(
        skills.len(),
        &[],
        &[],
        &zeros,
        &zeros,
        &[],
        &[],
        &HashMap::new(),
        &id_to_idx,
    );
    let with_bm25 = fuse_ranks(
        skills.len(),
        &[],
        &[],
        &zeros,
        &zeros,
        &reverse_bm25,
        &[],
        &HashMap::new(),
        &id_to_idx,
    );
    assert!(
        with_bm25.iter().all(|row| row.bm25_rank.is_some()),
        "nonempty BM25 must contribute ranks"
    );
    assert_eq!(
        empty.iter().map(|r| r.idx).collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
    assert_eq!(
        with_bm25.iter().map(|r| r.idx).collect::<Vec<_>>(),
        vec![4, 3, 2, 1, 0],
        "empty vs nonempty BM25 must change fused order"
    );

    let inv = WorkspaceInventory::default();
    let mock = mock_bm25_order(&skills, "please commit changes now");
    assert!(
        !mock.is_empty(),
        "paraphrase-adjacent query must produce a BM25 list"
    );
    let (local_scores, path_scores, local_order, path_order) =
        evidence_lists(&skills, "please commit changes now", &inv);
    let with_mock = fuse_ranks(
        skills.len(),
        &local_order,
        &path_order,
        &local_scores,
        &path_scores,
        &mock,
        &[],
        &HashMap::new(),
        &id_to_idx,
    );
    assert!(
        with_mock.iter().any(|row| row.bm25_rank.is_some()),
        "mock BM25 must contribute on a real query"
    );
}
