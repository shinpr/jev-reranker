#![deny(clippy::allow_attributes)]
#![deny(clippy::allow_attributes_without_reason)]
#![cfg_attr(
    test,
    allow(
        clippy::indexing_slicing,
        clippy::panic,
        clippy::unwrap_used,
        clippy::expect_used,
        reason = "Unit-test helpers use concise assertions; production paths remain strict."
    )
)]

mod compression;
mod error;
mod http;
mod options;
mod preparation;
mod ranking;
mod serialization;
mod skills;

use std::io::{self, Read, Write};

use clap::Parser;

use crate::error::AppError;
use crate::http::score_documents;
use crate::options::{resolve_options, CliOptions, Command};
use crate::preparation::{parse_input, prepare_documents};
use crate::ranking::rank_documents;
use crate::serialization::serialize_output;

fn main() {
    let cli = CliOptions::parse();
    let result = match &cli.command {
        Some(Command::Skills(command)) => skills::run(command),
        None => run(cli),
    };
    match result {
        Ok(output) => {
            if let Err(source) = io::stdout().write_all(&output) {
                eprintln!("{}", AppError::Io { source });
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(error.exit_code());
        }
    }
}

fn run(cli: CliOptions) -> Result<Vec<u8>, AppError> {
    let options = resolve_options(cli)?;
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|source| AppError::Io { source })?;
    let objects = parse_input(&input)?;
    let prepared = prepare_documents(objects, &options)?;
    let scores = score_documents(&prepared, &options)?;
    let ranked = rank_documents(prepared, &scores, &options)?;
    serialize_output(&ranked)
}
