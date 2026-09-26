use clap::{Parser, Subcommand, ValueEnum};

use crate::error::AppError;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Mode {
    Rerank,
    Filter,
    Compress,
}

#[derive(Clone, Debug, Parser, PartialEq)]
#[command(
    name = "jev-reranker",
    version,
    about = "Rerank, filter, or compress JSON search results with TypeSafe AI's Jev",
    after_help = "Reads a JSON array of objects from stdin and writes a JSON array to stdout.\nRequests need TYPESAFE_API_KEY in the environment.",
    subcommand_negates_reqs = true,
    args_conflicts_with_subcommands = true
)]
pub struct CliOptions {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Query used to judge relevance.
    #[arg(long, required = true)]
    pub query: Option<String>,

    /// Object field containing the text to score.
    #[arg(long, default_value = "text")]
    pub text_field: String,

    /// Field prepended to the text as context. May be repeated.
    #[arg(long = "context-field")]
    pub context_fields: Vec<String>,

    /// rerank sorts by relevance, filter drops candidates without usable evidence, compress keeps
    /// relevant sentences and lines.
    #[arg(long, default_value = "rerank")]
    pub mode: Mode,

    /// Minimum score, from 0 to 1, to keep a candidate or sentence. Filter and compress only
    /// (default: 0.5).
    #[arg(long)]
    pub threshold: Option<f64>,

    /// Maximum number of objects to output.
    #[arg(long)]
    pub top: Option<usize>,

    /// Jev model. Name a version such as jev-1.13.0 to keep scores stable across releases.
    #[arg(long, default_value = "jev-latest")]
    pub model: String,

    // Hidden because users cannot pick a better value; kept so tests can force several batches.
    #[arg(long, default_value_t = 30, hide = true)]
    pub batch_size: usize,

    /// Timeout for each HTTP attempt in milliseconds, not the whole run.
    #[arg(long, default_value_t = 10_000)]
    pub timeout_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Subcommand)]
pub enum Command {
    /// Manage the agent skill that teaches coding assistants to use this CLI.
    #[command(subcommand)]
    Skills(crate::skills::SkillsCommand),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedOptions {
    pub query: String,
    pub text_field: String,
    pub context_fields: Vec<String>,
    pub mode: Mode,
    pub threshold: f64,
    pub top: Option<usize>,
    pub model: String,
    pub batch_size: usize,
    pub timeout_ms: u64,
}

pub fn resolve_options(raw: CliOptions) -> Result<ResolvedOptions, AppError> {
    let query = raw.query.unwrap_or_default();
    if query.is_empty() {
        return Err(usage("query", "must not be empty"));
    }
    validate_name("text-field", &raw.text_field)?;
    for field in &raw.context_fields {
        validate_name("context-field", field)?;
    }
    validate_name("model", &raw.model)?;
    let threshold = raw.threshold.unwrap_or(0.5);
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        return Err(usage("threshold", "must be finite and between 0 and 1"));
    }
    if raw.mode == Mode::Rerank && raw.threshold.is_some() {
        return Err(usage("threshold", "requires --mode filter or compress"));
    }
    let output_field = match raw.mode {
        Mode::Rerank => "rerankScore",
        Mode::Filter => "evidenceScore",
        Mode::Compress => "compressedText",
    };
    if raw.text_field == output_field || raw.context_fields.iter().any(|f| f == output_field) {
        return Err(usage(
            "text-field/context-field",
            "must not use this mode's output field",
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
        query,
        text_field: raw.text_field,
        context_fields: raw.context_fields,
        mode: raw.mode,
        threshold,
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
    use super::*;

    #[test]
    fn validates_mode_specific_options_and_bounds() {
        for args in [
            vec!["cli", "--query", "q", "--threshold", "0.5"],
            vec![
                "cli",
                "--query",
                "q",
                "--mode",
                "filter",
                "--threshold",
                "NaN",
            ],
            vec![
                "cli",
                "--query",
                "q",
                "--mode",
                "compress",
                "--threshold",
                "1.1",
            ],
            vec!["cli", "--query", "q", "--top", "0"],
            vec!["cli", "--query", "q", "--batch-size", "31"],
            vec!["cli", "--query", "q", "--timeout-ms", "0"],
            vec!["cli", "--query", "q", "--text-field", "rerankScore"],
        ] {
            assert!(resolve_options(CliOptions::try_parse_from(args).unwrap()).is_err());
        }
        let options = resolve_options(CliOptions::parse_from(["cli", "--query", "q"])).unwrap();
        assert_eq!(options.mode, Mode::Rerank);
    }
}
