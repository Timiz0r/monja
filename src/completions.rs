use std::{collections::BTreeSet, fs, path::PathBuf};

use clap::{Args, CommandFactory};
use clap_complete::engine::CompletionCandidate;

use crate::Cli;

pub fn init() {
    clap_complete::CompleteEnv::with_factory(Cli::command).complete();
}

fn load_config() -> Option<monja::MonjaProfileConfig> {
    let base = xdg::BaseDirectories::with_prefix("monja");
    let profile_path = base.find_config_file("monja-profile.toml")?;

    fs::read_to_string(&profile_path)
        .ok()
        .and_then(|s| toml::from_str::<monja::MonjaProfileConfig>(&s).ok())
}

// completions run without a fully constructed profile, so this resolves repo dirs the cheap way
// rather than going through MonjaProfile -- a profile that fails to resolve should degrade to no
// completions, not an error.
fn repo_dirs(config: &monja::MonjaProfileConfig) -> Vec<PathBuf> {
    let home = std::env::home_dir();
    let resolve = |dir: &PathBuf| -> PathBuf {
        match dir.is_relative() {
            true => home
                .as_ref()
                .map(|home| home.join(dir))
                .unwrap_or_else(|| dir.clone()),
            false => dir.clone(),
        }
    };

    config
        .repo_dir
        .iter()
        .chain(config.repos.values())
        .map(resolve)
        .collect()
}

pub fn set_names() -> Vec<CompletionCandidate> {
    let Some(config) = load_config() else {
        return Vec::new();
    };

    // a BTreeSet because the same name can legitimately turn up in several repos, and a
    // duplicated completion candidate helps nobody. it also keeps the order stable.
    let mut names = BTreeSet::new();
    for repo_dir in repo_dirs(&config) {
        let Ok(entries) = fs::read_dir(&repo_dir) else {
            continue;
        };

        names.extend(
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .filter_map(|e| e.file_name().into_string().ok())
                //namely .git. no sane person would add a set starting with a dot
                .filter(|name| !name.starts_with('.')),
        );
    }

    names.into_iter().map(CompletionCandidate::new).collect()
}

pub fn repo_names() -> Vec<CompletionCandidate> {
    let Some(config) = load_config() else {
        return Vec::new();
    };

    match config.repos.is_empty() {
        // the single-repo `repo-dir` form has an implicit name rather than a configured one
        true => match config.repo_dir.is_some() {
            true => vec![CompletionCandidate::new(monja::RepoName::DEFAULT)],
            false => Vec::new(),
        },
        false => config
            .repos
            .keys()
            .map(|name| CompletionCandidate::new(&name.0))
            .collect(),
    }
}

#[derive(Args)]
pub struct CompletionsCommand {}
impl CompletionsCommand {
    pub fn execute(&self) -> anyhow::Result<()> {
        let shell = clap_complete::Shell::from_env()
            .ok_or(anyhow::anyhow!("Unable to determine shell."))?;
        // SAFETY: this runs single-threaded before any completion work begins
        unsafe { std::env::set_var("COMPLETE", shell.to_string()) };
        clap_complete::CompleteEnv::with_factory(Cli::command).complete();
        Ok(())
    }
}
