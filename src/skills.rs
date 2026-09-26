use std::env;
use std::fs;
use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::error::AppError;

// The skill ships inside the binary so an installed skill always matches this CLI's options.
const SKILL: &str = include_str!("../skills/jev-reranker/SKILL.md");
const SKILL_NAME: &str = "jev-reranker";

#[derive(Clone, Debug, PartialEq, Subcommand)]
pub enum SkillsCommand {
    /// Install the jev-reranker agent skill.
    Install(InstallArgs),
}

#[derive(Clone, Debug, PartialEq, Args)]
#[command(group(clap::ArgGroup::new("target").required(true).args(["claude_code", "codex", "path"])))]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Each bool is an independent command-line flag."
)]
pub struct InstallArgs {
    /// Install for Claude Code, in ./.claude/skills unless --global is given.
    #[arg(long)]
    pub claude_code: bool,

    /// Install for Codex, in $CODEX_HOME/skills (or ~/.codex/skills) unless --project is given.
    #[arg(long)]
    pub codex: bool,

    /// Install for the current user instead of this project.
    #[arg(long, conflicts_with_all = ["project", "path"])]
    pub global: bool,

    /// Install into this project instead of the user's home.
    #[arg(long, conflicts_with = "path")]
    pub project: bool,

    /// Install into <PATH>/jev-reranker.
    #[arg(long, value_name = "PATH")]
    pub path: Option<PathBuf>,
}

pub fn run(command: &SkillsCommand) -> Result<Vec<u8>, AppError> {
    let SkillsCommand::Install(args) = command;
    let directory = target_directory(args)?;
    let file = directory.join("SKILL.md");
    fs::create_dir_all(&directory)
        .and_then(|()| fs::write(&file, SKILL))
        .map_err(|source| AppError::SkillInstall {
            path: file.display().to_string(),
            source,
        })?;
    Ok(format!("Installed the jev-reranker skill to {}\n", file.display()).into_bytes())
}

fn target_directory(args: &InstallArgs) -> Result<PathBuf, AppError> {
    if let Some(path) = &args.path {
        return Ok(path.join(SKILL_NAME));
    }
    let root = if args.codex {
        if args.project {
            PathBuf::from(".codex")
        } else {
            env::var_os("CODEX_HOME")
                .filter(|value| !value.is_empty())
                .map_or_else(
                    || home().map(|home| home.join(".codex")),
                    |value| Ok(PathBuf::from(value)),
                )?
        }
    } else if args.global {
        home()?.join(".claude")
    } else {
        PathBuf::from(".claude")
    };
    Ok(root.join("skills").join(SKILL_NAME))
}

fn home() -> Result<PathBuf, AppError> {
    let variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(AppError::MissingHome { variable })
}
