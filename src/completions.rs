use std::fs;

use clap::{Args, CommandFactory};
use clap_complete::engine::CompletionCandidate;

use crate::Cli;

pub fn init() {
    clap_complete::CompleteEnv::with_factory(Cli::command).complete();
}

pub fn set_names() -> Vec<CompletionCandidate> {
    let base = xdg::BaseDirectories::with_prefix("monja");
    let profile_path = match base.find_config_file("monja-profile.toml") {
        Some(path) => path,
        None => return Vec::new(),
    };

    let config = match fs::read_to_string(&profile_path)
        .ok()
        .and_then(|s| toml::from_str::<monja::MonjaProfileConfig>(&s).ok())
    {
        Some(config) => config,
        None => return Vec::new(),
    };

    let repo_dir = if config.repo_dir.is_relative() {
        std::env::home_dir()
            .map(|home| home.join(&config.repo_dir))
            .unwrap_or(config.repo_dir)
    } else {
        config.repo_dir
    };

    let Ok(entries) = fs::read_dir(&repo_dir) else {
        return Vec::new();
    };

    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| !name.starts_with('.')) //namely .git. no sane person would add a set starting with a dot
        .map(CompletionCandidate::new)
        .collect()
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
