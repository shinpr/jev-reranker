use clap::{Parser, ValueEnum};

use crate::error::AppError;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ScoreOrder {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum FusionMode {
    Boost,
    RerankOnly,
}

#[derive(Clone, Debug, Parser, PartialEq)]
#[command(name = "jev-reranker")]
pub struct CliOptions {
    #[arg(long)]
    pub query: String,

    #[arg(long, default_value = "text")]
    pub text_field: String,

    #[arg(long = "context-field")]
    pub context_fields: Vec<String>,

    #[arg(long)]
    pub score_field: Option<String>,

    #[arg(long)]
    pub score_order: Option<ScoreOrder>,

    #[arg(long)]
    pub fusion: Option<FusionMode>,

    #[arg(long)]
    pub weight: Option<f64>,

    #[arg(long)]
    pub top: Option<usize>,

    #[arg(long, default_value = "jev-latest")]
    pub model: String,

    #[arg(long, default_value_t = 30)]
    pub batch_size: usize,

    #[arg(long, default_value_t = 10_000)]
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedOptions {
    pub query: String,
    pub text_field: String,
    pub context_fields: Vec<String>,
    pub score_field: Option<String>,
    pub score_order: Option<ScoreOrder>,
    pub fusion: FusionMode,
    pub weight: f64,
    pub top: Option<usize>,
    pub model: String,
    pub batch_size: usize,
    pub timeout_ms: u64,
}

pub fn resolve_options(raw: CliOptions) -> Result<ResolvedOptions, AppError> {
    if raw.query.is_empty() {
        return Err(usage("query", "must not be empty"));
    }
    validate_name("text-field", &raw.text_field)?;
    for field in &raw.context_fields {
        validate_name("context-field", field)?;
    }
    if let Some(field) = &raw.score_field {
        validate_name("score-field", field)?;
    }
    if raw.model.is_empty() {
        return Err(usage("model", "must not be empty"));
    }

    match (&raw.score_field, raw.score_order) {
        (Some(_), None) => return Err(usage("score-order", "is required with --score-field")),
        (None, Some(_)) => return Err(usage("score-order", "requires --score-field")),
        _ => {}
    }

    let fusion = raw.fusion.unwrap_or(match raw.score_field {
        Some(_) => FusionMode::Boost,
        None => FusionMode::RerankOnly,
    });
    if fusion == FusionMode::Boost && raw.score_field.is_none() {
        return Err(usage(
            "fusion",
            "boost requires --score-field and --score-order",
        ));
    }

    let weight = raw.weight.unwrap_or(1.0);
    if !weight.is_finite() || weight < 0.0 {
        return Err(usage("weight", "must be finite and at least zero"));
    }
    if fusion == FusionMode::RerankOnly && raw.weight.is_some() {
        return Err(usage(
            "weight",
            "cannot be supplied with --fusion rerank-only",
        ));
    }

    if raw.top == Some(0) {
        return Err(usage("top", "must be at least 1"));
    }
    if !(1..=30).contains(&raw.batch_size) {
        return Err(usage("batch-size", "must be between 1 and 30"));
    }
    if raw.timeout_ms == 0 {
        return Err(usage("timeout-ms", "must be greater than zero"));
    }

    Ok(ResolvedOptions {
        query: raw.query,
        text_field: raw.text_field,
        context_fields: raw.context_fields,
        score_field: raw.score_field,
        score_order: raw.score_order,
        fusion,
        weight,
        top: raw.top,
        model: raw.model,
        batch_size: raw.batch_size,
        timeout_ms: raw.timeout_ms,
    })
}

fn validate_name(option: &str, value: &str) -> Result<(), AppError> {
    if value.is_empty() {
        return Err(usage(option, "must not be empty"));
    }
    Ok(())
}

fn usage(option: &str, message: &str) -> AppError {
    AppError::Usage {
        option: option.to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_options, CliOptions, FusionMode, ScoreOrder};

    fn base() -> CliOptions {
        CliOptions {
            query: "query".to_owned(),
            text_field: "text".to_owned(),
            context_fields: Vec::new(),
            score_field: None,
            score_order: None,
            fusion: None,
            weight: None,
            top: None,
            model: "jev-latest".to_owned(),
            batch_size: 30,
            timeout_ms: 10_000,
        }
    }

    #[test]
    fn resolves_rerank_only_defaults_without_score_options() {
        let resolved = resolve_options(base());
        assert!(resolved.is_ok());
        if let Ok(options) = resolved {
            assert_eq!(options.fusion, FusionMode::RerankOnly);
            assert!((options.weight - 1.0).abs() < f64::EPSILON);
            assert_eq!(options.batch_size, 30);
            assert_eq!(options.timeout_ms, 10_000);
        }
    }

    #[test]
    fn resolves_boost_defaults_when_score_options_are_present() {
        let mut input = base();
        input.score_field = Some("score".to_owned());
        input.score_order = Some(ScoreOrder::Asc);
        let resolved = resolve_options(input);
        assert!(resolved.is_ok());
        if let Ok(options) = resolved {
            assert_eq!(options.fusion, FusionMode::Boost);
            assert_eq!(options.score_order, Some(ScoreOrder::Asc));
        }
    }

    #[test]
    fn rejects_relationships_and_bounds_with_usage_errors() {
        let mut missing_order = base();
        missing_order.score_field = Some("score".to_owned());
        let missing_order_error = resolve_options(missing_order);
        assert!(matches!(
            missing_order_error,
            Err(crate::error::AppError::Usage { .. })
        ));

        let mut zero_top = base();
        zero_top.top = Some(0);
        let zero_top_error = resolve_options(zero_top);
        assert!(matches!(
            zero_top_error,
            Err(crate::error::AppError::Usage { .. })
        ));

        let mut explicit_weight = base();
        explicit_weight.weight = Some(2.0);
        let explicit_weight_error = resolve_options(explicit_weight);
        assert!(matches!(
            explicit_weight_error,
            Err(crate::error::AppError::Usage { .. })
        ));

        let mut invalid_batch = base();
        invalid_batch.batch_size = 31;
        let invalid_batch_error = resolve_options(invalid_batch);
        assert!(matches!(
            invalid_batch_error,
            Err(crate::error::AppError::Usage { .. })
        ));
    }

    #[test]
    fn accepts_explicit_rerank_only_with_score_options_but_ignores_source_score() {
        let mut input = base();
        input.score_field = Some("score".to_owned());
        input.score_order = Some(ScoreOrder::Desc);
        input.fusion = Some(FusionMode::RerankOnly);
        let resolved = resolve_options(input);
        assert!(resolved.is_ok());
        if let Ok(options) = resolved {
            assert_eq!(options.fusion, FusionMode::RerankOnly);
        }
    }
}
