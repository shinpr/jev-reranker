use serde_json::{Map, Value};

use crate::error::AppError;
use crate::options::{Mode, ResolvedOptions};
use crate::preparation::PreparedDocument;

pub fn rank_documents(
    documents: Vec<PreparedDocument>,
    scores: &[f64],
    options: &ResolvedOptions,
) -> Result<Vec<Map<String, Value>>, AppError> {
    let expected = documents
        .iter()
        .map(|doc| match options.mode {
            Mode::Compress => doc.units.len(),
            Mode::Rerank | Mode::Filter => 1,
        })
        .sum();
    if expected != scores.len() {
        return Err(AppError::ScoreCountMismatch {
            expected,
            actual: scores.len(),
        });
    }
    for (index, score) in scores.iter().enumerate() {
        if !score.is_finite() || !(0.0..=1.0).contains(score) {
            return Err(AppError::InvalidRerankScore { index });
        }
    }
    let mut results = match options.mode {
        Mode::Rerank => rerank(documents, scores),
        Mode::Filter => filter(documents, scores, options.threshold),
        Mode::Compress => compress(documents, scores, options.threshold),
    };
    if let Some(top) = options.top {
        results.truncate(top);
    }
    Ok(results)
}

fn rerank(documents: Vec<PreparedDocument>, scores: &[f64]) -> Vec<Map<String, Value>> {
    let mut ranked = documents
        .into_iter()
        .zip(scores.iter().copied())
        .collect::<Vec<_>>();
    // Stable sort preserves input order on ties, including equal signed zeros.
    ranked.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked
        .into_iter()
        .map(|(document, score)| {
            let mut object = document.object;
            object.insert("rerankScore".to_owned(), serde_json::json!(score));
            object
        })
        .collect()
}

fn filter(
    documents: Vec<PreparedDocument>,
    scores: &[f64],
    threshold: f64,
) -> Vec<Map<String, Value>> {
    documents
        .into_iter()
        .zip(scores.iter().copied())
        .filter(|(_, score)| *score >= threshold)
        .map(|(document, score)| {
            let mut object = document.object;
            object.insert("evidenceScore".to_owned(), serde_json::json!(score));
            object
        })
        .collect()
}

fn compress(
    documents: Vec<PreparedDocument>,
    scores: &[f64],
    threshold: f64,
) -> Vec<Map<String, Value>> {
    let mut results = Vec::new();
    let mut remaining = scores.iter().copied();
    for document in documents {
        let mut passages = Vec::new();
        let mut passage = String::new();
        for (unit, score) in document
            .units
            .iter()
            .zip(remaining.by_ref().take(document.units.len()))
        {
            if score >= threshold {
                passage.push_str(unit);
            } else if !passage.is_empty() {
                passages.push(passage.trim().to_owned());
                passage.clear();
            }
        }
        if !passage.is_empty() {
            passages.push(passage.trim().to_owned());
        }
        if !passages.is_empty() {
            let mut object = document.object;
            object.insert(
                "compressedText".to_owned(),
                Value::String(passages.join("\n")),
            );
            results.push(object);
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{resolve_options, CliOptions};
    use crate::preparation::{parse_input, prepare_documents};
    use clap::Parser;

    #[test]
    fn rerank_preserves_ties_metadata_and_applies_top_after_sorting() {
        let options = resolve_options(CliOptions::parse_from([
            "cli", "--query", "q", "--top", "2",
        ]))
        .unwrap();
        let docs = prepare_documents(
            parse_input(
                r#"[{"text":"a","score":-7},{"text":"b","rerankScore":"old"},{"text":"c"}]"#,
            )
            .unwrap(),
            &options,
        )
        .unwrap();
        let result = rank_documents(docs, &[0.1, 0.8, 0.8], &options).unwrap();
        assert_eq!(result[0]["text"], "b");
        assert_eq!(result[1]["text"], "c");
        assert_eq!(result[0]["rerankScore"], serde_json::json!(0.8));
    }

    #[test]
    fn selection_top_limits_survivors_without_reordering() {
        for mode in ["filter", "compress"] {
            let options = resolve_options(CliOptions::parse_from([
                "cli", "--query", "q", "--mode", mode, "--top", "1",
            ]))
            .unwrap();
            let docs = prepare_documents(
                parse_input(
                    r#"[{"text":"skip"},{"text":"first evidence"},{"text":"higher score"}]"#,
                )
                .unwrap(),
                &options,
            )
            .unwrap();
            let result = rank_documents(docs.clone(), &[0.1, 0.5, 0.9], &options).unwrap();
            assert_eq!(result.len(), 1);
            assert_eq!(result[0]["text"], "first evidence");
            assert!(rank_documents(docs, &[0.1, 0.2, 0.3], &options)
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn compression_joins_adjacent_fragments_but_separates_removed_passages() {
        let options = resolve_options(CliOptions::parse_from([
            "cli", "--query", "q", "--mode", "compress",
        ]))
        .unwrap();
        let docs = prepare_documents(
            parse_input(r#"[{"text":"Silas B. Cobb paid. Unrelated text. Approval required."}]"#)
                .unwrap(),
            &options,
        )
        .unwrap();
        let result = rank_documents(docs, &[0.9, 0.9, 0.1, 0.9], &options).unwrap();
        assert_eq!(
            result[0]["compressedText"],
            "Silas B. Cobb paid.\nApproval required."
        );
    }

    #[test]
    fn rejects_mismatched_or_non_probability_scores() {
        let options = resolve_options(CliOptions::parse_from(["cli", "--query", "q"])).unwrap();
        let docs = prepare_documents(parse_input(r#"[{"text":"a"}]"#).unwrap(), &options).unwrap();
        for scores in [vec![], vec![f64::NAN], vec![1.1], vec![-0.1]] {
            assert!(rank_documents(docs.clone(), &scores, &options).is_err());
        }
    }
}
