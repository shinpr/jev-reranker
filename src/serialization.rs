use serde_json::{Map, Value};

use crate::error::AppError;

pub fn serialize_output(objects: &[Map<String, Value>]) -> Result<Vec<u8>, AppError> {
    let mut output =
        serde_json::to_vec(objects).map_err(|source| AppError::Serialization { source })?;
    output.push(b'\n');
    Ok(output)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    use super::serialize_output;
    use crate::options::{resolve_options, CliOptions, FusionMode};
    use crate::preparation::{parse_input, prepare_documents};
    use crate::ranking::rank_documents;

    fn object(value: Value) -> Map<String, Value> {
        let Value::Object(map) = value else {
            panic!("test value must be an object");
        };
        map
    }

    #[test]
    fn serializes_to_an_atomic_newline_terminated_buffer() {
        let objects = vec![object(json!({
            "id": "one",
            "unknown": 18_446_744_073_709_551_617_i128
        }))];
        let serialized = serialize_output(&objects);
        assert!(serialized.is_ok());
        if let Ok(bytes) = serialized {
            assert!(bytes.ends_with(b"\n"));
            let body = bytes.strip_suffix(b"\n");
            assert!(body.is_some());
            if let Some(body) = body {
                assert!(!body.contains(&b'\n'));
            }
            let parsed: Result<Value, _> = serde_json::from_slice(&bytes);
            assert!(parsed.is_ok());
            if let Ok(value) = parsed {
                assert_eq!(
                    value,
                    json!([{
                        "id": "one",
                        "unknown": 18_446_744_073_709_551_617_i128
                    }])
                );
            }
        }
    }

    #[test]
    fn preserves_an_unknown_integer_through_the_pure_pipeline() {
        let raw = r#"[{"text":"body","futureInteger":18446744073709551617}]"#;
        let parsed = parse_input(raw);
        assert!(parsed.is_ok());
        if let Ok(objects) = parsed {
            let options = resolve_options(CliOptions {
                query: "q".to_owned(),
                text_field: "text".to_owned(),
                context_fields: Vec::new(),
                score_field: None,
                score_order: None,
                fusion: Some(FusionMode::RerankOnly),
                weight: None,
                top: None,
                model: "jev-latest".to_owned(),
                batch_size: 30,
                timeout_ms: 10_000,
            });
            assert!(options.is_ok());
            if let Ok(options) = options {
                let prepared = prepare_documents(objects, &options);
                assert!(prepared.is_ok());
                if let Ok(prepared) = prepared {
                    let ranked = rank_documents(prepared, &[0.75], &options);
                    assert!(ranked.is_ok());
                    if let Ok(output) = ranked {
                        let serialized = serialize_output(&output);
                        assert!(serialized.is_ok());
                        if let Ok(bytes) = serialized {
                            let parsed_output: Result<Value, _> = serde_json::from_slice(&bytes);
                            assert!(parsed_output.is_ok());
                            if let Ok(value) = parsed_output {
                                let integer =
                                    value.pointer("/0/futureInteger").map(Value::to_string);
                                assert_eq!(integer.as_deref(), Some("18446744073709551617"));
                            }
                        }
                    }
                }
            }
        }
    }
}
