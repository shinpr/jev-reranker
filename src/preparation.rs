use serde_json::{Map, Value};

use crate::error::AppError;
use crate::options::{Mode, ResolvedOptions};

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedDocument {
    pub original_index: usize,
    pub object: Map<String, Value>,
    pub prepared_text: String,
    pub units: Vec<String>,
}

pub fn parse_input(input: &str) -> Result<Vec<Map<String, Value>>, AppError> {
    let value: Value = serde_json::from_str(input).map_err(|source| AppError::Json { source })?;
    let Value::Array(values) = value else {
        return Err(AppError::InvalidInput {
            message: "expected a JSON array".to_owned(),
        });
    };
    let mut objects = Vec::with_capacity(values.len());
    for (index, value) in values.into_iter().enumerate() {
        let Value::Object(object) = value else {
            return Err(AppError::InvalidItem {
                index,
                field: "<object>".to_owned(),
                message: "must be a JSON object".to_owned(),
            });
        };
        objects.push(object);
    }
    Ok(objects)
}

pub fn prepare_documents(
    objects: Vec<Map<String, Value>>,
    options: &ResolvedOptions,
) -> Result<Vec<PreparedDocument>, AppError> {
    let mut prepared = Vec::with_capacity(objects.len());
    for (original_index, object) in objects.into_iter().enumerate() {
        let text = match object.get(&options.text_field) {
            Some(Value::String(text)) => text.clone(),
            Some(
                Value::Null
                | Value::Bool(_)
                | Value::Number(_)
                | Value::Array(_)
                | Value::Object(_),
            ) => {
                return Err(AppError::InvalidItem {
                    index: original_index,
                    field: options.text_field.clone(),
                    message: "must be a string".to_owned(),
                });
            }
            None => {
                return Err(AppError::InvalidItem {
                    index: original_index,
                    field: options.text_field.clone(),
                    message: "is required".to_owned(),
                });
            }
        };

        let mut text_parts = Vec::with_capacity(options.context_fields.len() + 1);
        for field in &options.context_fields {
            match object.get(field) {
                None | Some(Value::Null) => {}
                Some(Value::String(value)) => text_parts.push(value.clone()),
                Some(Value::Bool(_) | Value::Number(_) | Value::Array(_) | Value::Object(_)) => {
                    return Err(AppError::InvalidItem {
                        index: original_index,
                        field: field.clone(),
                        message: "must be a string or null".to_owned(),
                    });
                }
            }
        }
        text_parts.push(text.clone());
        let prepared_text = text_parts.join("\n\n");

        let units = if options.mode == Mode::Compress {
            crate::compression::split_units(&text, &options.language)
        } else {
            Vec::new()
        };

        prepared.push(PreparedDocument {
            original_index,
            object,
            prepared_text,
            units,
        });
    }
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Map, Value};

    use super::{parse_input, prepare_documents};
    use crate::options::{resolve_options, CliOptions};
    use clap::Parser;

    fn options() -> crate::options::ResolvedOptions {
        let raw = CliOptions::parse_from([
            "cli",
            "--query",
            "q",
            "--context-field",
            "title",
            "--context-field",
            "section",
        ]);
        match resolve_options(raw) {
            Ok(value) => value,
            Err(error) => panic!("test options must resolve: {error}"),
        }
    }

    fn object(value: Value) -> Map<String, Value> {
        let Value::Object(map) = value else {
            panic!("test value must be an object");
        };
        map
    }

    #[test]
    fn prepares_exact_context_text_and_preserves_the_original_map() {
        let input = vec![object(json!({
            "title": "Guide",
            "section": "Setup",
            "text": "Use this value",
            "score": 3.0,
            "future": {"enabled": true},
            "huge": 18_446_744_073_709_551_617_i128
        }))];
        let prepared = prepare_documents(input, &options());
        assert!(prepared.is_ok());
        if let Ok(documents) = prepared {
            assert_eq!(documents.len(), 1);
            if let Some(document) = documents.first() {
                assert_eq!(document.prepared_text, "Guide\n\nSetup\n\nUse this value");
                assert_eq!(document.original_index, 0);

                assert!(document.object.contains_key("future"));
                assert!(document.object.contains_key("huge"));
            }
        }
    }

    #[test]
    fn skips_missing_and_null_context_but_rejects_non_strings() {
        let mut null_context = object(json!({"text": "body", "title": null, "score": 1.0}));
        let prepared = prepare_documents(vec![null_context.clone()], &options());
        assert!(prepared.is_ok());
        if let Ok(documents) = prepared {
            if let Some(document) = documents.first() {
                assert_eq!(document.prepared_text, "body");
            }
        }

        null_context.insert("title".to_owned(), json!(false));
        let rejected = prepare_documents(vec![null_context], &options());
        assert!(matches!(
            rejected,
            Err(crate::error::AppError::InvalidItem { .. })
        ));
    }

    #[test]
    fn validates_object_array_and_required_text_field() {
        let parsed = parse_input(r#"[{"text":"one"},{"text":"two"}]"#);
        assert!(parsed.is_ok());
        if let Ok(objects) = parsed {
            assert_eq!(objects.len(), 2);
        }

        let scalar = parse_input(r#"[{"text":"ok"},3]"#);
        assert!(matches!(
            scalar,
            Err(crate::error::AppError::InvalidItem { .. })
        ));

        let missing_text =
            prepare_documents(vec![object(json!({"title": "only context"}))], &options());
        assert!(matches!(
            missing_text,
            Err(crate::error::AppError::InvalidItem { .. })
        ));

        let malformed = parse_input("[");
        assert!(matches!(
            malformed,
            Err(crate::error::AppError::Json { .. })
        ));
    }
}
