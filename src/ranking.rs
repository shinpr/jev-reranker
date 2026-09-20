use std::cmp::Ordering;

use serde_json::{Map, Value};

use crate::error::AppError;
use crate::options::{FusionMode, ResolvedOptions, ScoreOrder};
use crate::preparation::PreparedDocument;

pub fn rank_documents(
    documents: Vec<PreparedDocument>,
    rerank_scores: &[f64],
    options: &ResolvedOptions,
) -> Result<Vec<Map<String, Value>>, AppError> {
    if documents.len() != rerank_scores.len() {
        return Err(AppError::ScoreCountMismatch {
            expected: documents.len(),
            actual: rerank_scores.len(),
        });
    }

    let mut ranked = Vec::with_capacity(documents.len());
    for (document, rerank_score) in documents.into_iter().zip(rerank_scores.iter().copied()) {
        ranked.push(build_ranked_document(document, rerank_score, options)?);
    }

    sort_ranked_documents(&mut ranked, options)?;

    if let Some(top) = options.top {
        ranked.truncate(top);
    }

    render_ranked_documents(ranked)
}

fn build_ranked_document(
    document: PreparedDocument,
    rerank_score: f64,
    options: &ResolvedOptions,
) -> Result<RankedDocument, AppError> {
    if !rerank_score.is_finite() || !(0.0..=1.0).contains(&rerank_score) {
        return Err(AppError::InvalidRerankScore {
            index: document.original_index,
        });
    }

    let fused_score = if options.fusion == FusionMode::Boost {
        let score_field = options.score_field.as_deref().unwrap_or("<score>");
        let Some(source_score) = document.source_score else {
            return Err(AppError::InvalidItem {
                index: document.original_index,
                field: score_field.to_owned(),
                message: "is required in boost mode".to_owned(),
            });
        };
        let factor = 1.0 + rerank_score * options.weight;
        let fused = match options.score_order {
            Some(ScoreOrder::Asc) => source_score / factor,
            Some(ScoreOrder::Desc) => source_score * factor,
            None => {
                return Err(AppError::Usage {
                    option: "score-order".to_owned(),
                    message: "is required in boost mode".to_owned(),
                });
            }
        };
        if !fused.is_finite() {
            return Err(AppError::NonFiniteComputedScore {
                index: document.original_index,
            });
        }
        Some(fused)
    } else {
        None
    };
    let rank_key = fused_score.unwrap_or(rerank_score);
    Ok(RankedDocument {
        document,
        rerank_score,
        fused_score,
        rank_key,
    })
}

fn sort_ranked_documents(
    ranked: &mut [RankedDocument],
    options: &ResolvedOptions,
) -> Result<(), AppError> {
    let tie_break = |left: &RankedDocument, right: &RankedDocument| {
        left.document
            .original_index
            .cmp(&right.document.original_index)
    };
    if options.fusion == FusionMode::Boost {
        match options.score_order {
            Some(ScoreOrder::Asc) => ranked.sort_by(|left, right| {
                compare_rank_keys(left.rank_key, right.rank_key)
                    .then_with(|| tie_break(left, right))
            }),
            Some(ScoreOrder::Desc) => ranked.sort_by(|left, right| {
                compare_rank_keys(right.rank_key, left.rank_key)
                    .then_with(|| tie_break(left, right))
            }),
            None => {
                return Err(AppError::Usage {
                    option: "score-order".to_owned(),
                    message: "is required in boost mode".to_owned(),
                });
            }
        }
    } else {
        ranked.sort_by(|left, right| {
            compare_rank_keys(right.rank_key, left.rank_key).then_with(|| tie_break(left, right))
        });
    }
    Ok(())
}

fn compare_rank_keys(left: f64, right: f64) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

fn render_ranked_documents(
    ranked: Vec<RankedDocument>,
) -> Result<Vec<Map<String, Value>>, AppError> {
    let mut output = Vec::with_capacity(ranked.len());
    for ranked_document in ranked {
        let index = ranked_document.document.original_index;
        let mut object = ranked_document.document.object;
        object.insert(
            "rerankScore".to_owned(),
            finite_json_value(ranked_document.rerank_score, index)?,
        );
        if let Some(fused_score) = ranked_document.fused_score {
            object.insert(
                "fusedScore".to_owned(),
                finite_json_value(fused_score, index)?,
            );
        }
        output.push(object);
    }
    Ok(output)
}

struct RankedDocument {
    document: PreparedDocument,
    rerank_score: f64,
    fused_score: Option<f64>,
    rank_key: f64,
}

fn finite_json_value(value: f64, index: usize) -> Result<Value, AppError> {
    match serde_json::Number::from_f64(value) {
        Some(number) => Ok(Value::Number(number)),
        None => Err(AppError::NonFiniteComputedScore { index }),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    use super::rank_documents;
    use crate::options::{resolve_options, CliOptions, FusionMode, ScoreOrder};
    use crate::preparation::{prepare_documents, PreparedDocument};

    fn object(value: Value) -> Map<String, Value> {
        let Value::Object(map) = value else {
            panic!("test value must be an object");
        };
        map
    }

    fn options(
        fusion: FusionMode,
        score_order: Option<ScoreOrder>,
        top: Option<usize>,
        weight: Option<f64>,
    ) -> crate::options::ResolvedOptions {
        let raw = CliOptions {
            query: "q".to_owned(),
            text_field: "text".to_owned(),
            context_fields: Vec::new(),
            score_field: (fusion == FusionMode::Boost).then_some("score".to_owned()),
            score_order,
            fusion: Some(fusion),
            weight,
            top,
            model: "jev-latest".to_owned(),
            batch_size: 30,
            timeout_ms: 10_000,
        };
        match resolve_options(raw) {
            Ok(value) => value,
            Err(error) => panic!("test options must resolve: {error}"),
        }
    }

    fn documents() -> Vec<PreparedDocument> {
        let raw = vec![
            object(json!({"id":"first", "text":"a", "score": 10.0, "fusedScore": "old"})),
            object(json!({"id":"second", "text":"b", "score": 5.0})),
            object(json!({"id":"third", "text":"c", "score": 5.0})),
        ];
        let prepared = prepare_documents(
            raw,
            &options(FusionMode::Boost, Some(ScoreOrder::Asc), None, Some(2.0)),
        );
        match prepared {
            Ok(value) => value,
            Err(error) => panic!("test documents must prepare: {error}"),
        }
    }

    #[test]
    fn applies_ascending_magnitude_preserving_fusion_and_replaces_collisions() {
        let ranked = rank_documents(
            documents(),
            &[0.5, 0.2, 0.2],
            &options(FusionMode::Boost, Some(ScoreOrder::Asc), None, Some(2.0)),
        );
        assert!(ranked.is_ok());
        if let Ok(output) = ranked {
            assert_eq!(output.len(), 3);
            if let Some(first) = output.first() {
                assert_eq!(first.get("id"), Some(&json!("second")));
                assert_eq!(first.get("rerankScore"), Some(&json!(0.2)));
                assert_eq!(first.get("fusedScore"), Some(&json!(5.0 / 1.4)));
            }
            if let Some(last) = output.last() {
                assert_eq!(last.get("id"), Some(&json!("first")));
                assert_eq!(last.get("fusedScore"), Some(&json!(10.0 / 2.0)));
            }
        }
    }

    #[test]
    fn applies_descending_fusion_and_accepts_negative_source_scores() {
        let raw = vec![
            object(json!({"id":"negative", "text":"a", "score": -4.0})),
            object(json!({"id":"positive", "text":"b", "score": 2.0})),
        ];
        let options = options(FusionMode::Boost, Some(ScoreOrder::Desc), None, Some(1.0));
        let prepared = prepare_documents(raw, &options);
        assert!(prepared.is_ok());
        if let Ok(documents) = prepared {
            let ranked = rank_documents(documents, &[0.5, 0.5], &options);
            assert!(ranked.is_ok());
            if let Ok(output) = ranked {
                if let Some(first) = output.first() {
                    assert_eq!(first.get("id"), Some(&json!("positive")));
                    assert_eq!(first.get("fusedScore"), Some(&json!(3.0)));
                }
                if let Some(last) = output.last() {
                    assert_eq!(last.get("id"), Some(&json!("negative")));
                    assert_eq!(last.get("fusedScore"), Some(&json!(-6.0)));
                }
            }
        }
    }

    #[test]
    fn keeps_input_order_for_equal_keys_and_applies_top_after_sort() {
        let options = options(FusionMode::RerankOnly, None, Some(2), None);
        let raw = vec![
            object(
                json!({"id":"first", "text":"a", "rerankScore":"old", "fusedScore": "passthrough"}),
            ),
            object(json!({"id":"second", "text":"b"})),
            object(json!({"id":"third", "text":"c"})),
        ];
        let prepared = prepare_documents(raw, &options);
        assert!(prepared.is_ok());
        if let Ok(documents) = prepared {
            let ranked = rank_documents(documents, &[0.5, 0.9, 0.5], &options);
            assert!(ranked.is_ok());
            if let Ok(output) = ranked {
                assert_eq!(output.len(), 2);
                if let Some(first) = output.first() {
                    assert_eq!(first.get("id"), Some(&json!("second")));
                    assert!(first.get("fusedScore").is_none());
                }
                if let Some(last) = output.last() {
                    assert_eq!(last.get("id"), Some(&json!("first")));
                    assert_eq!(last.get("rerankScore"), Some(&json!(0.5)));
                    assert_eq!(last.get("fusedScore"), Some(&json!("passthrough")));
                }
            }
        }
    }

    #[test]
    fn rejects_invalid_probability_and_score_cardinality() {
        let options = options(FusionMode::RerankOnly, None, None, None);
        let prepared = prepare_documents(vec![object(json!({"id":"one", "text":"a"}))], &options);
        assert!(prepared.is_ok());
        if let Ok(documents) = prepared {
            let invalid_probability = rank_documents(documents.clone(), &[1.1], &options);
            assert!(matches!(
                invalid_probability,
                Err(crate::error::AppError::InvalidRerankScore { .. })
            ));
            let mismatched = rank_documents(documents, &[], &options);
            assert!(matches!(
                mismatched,
                Err(crate::error::AppError::ScoreCountMismatch { .. })
            ));
        }
    }

    #[test]
    fn rejects_non_finite_rerank_probability_before_output() {
        let options = options(FusionMode::RerankOnly, None, None, None);
        let prepared = prepare_documents(vec![object(json!({"id": "one", "text": "a"}))], &options);
        assert!(prepared.is_ok());
        if let Ok(documents) = prepared {
            let rejected = rank_documents(documents, &[f64::NAN], &options);
            assert!(matches!(
                rejected,
                Err(crate::error::AppError::InvalidRerankScore { .. })
            ));
        }
    }

    #[test]
    fn treats_signed_zero_as_an_equal_key_for_stable_ties() {
        let options = options(FusionMode::RerankOnly, None, None, None);
        let prepared = prepare_documents(
            vec![
                object(json!({"id": "negative-zero", "text": "a"})),
                object(json!({"id": "positive-zero", "text": "b"})),
            ],
            &options,
        );
        assert!(prepared.is_ok());
        if let Ok(documents) = prepared {
            let ranked = rank_documents(documents, &[-0.0, 0.0], &options);
            assert!(ranked.is_ok());
            if let Ok(output) = ranked {
                if let Some(first) = output.first() {
                    assert_eq!(first.get("id"), Some(&json!("negative-zero")));
                }
                if let Some(last) = output.last() {
                    assert_eq!(last.get("id"), Some(&json!("positive-zero")));
                }
            }
        }
    }
}
