use std::{fs::File, path::PathBuf};

use clap::{Args, Parser, Subcommand, command};
use serde::{Deserialize, Serialize};

use monja::MonjaProfile;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Push(PushCommand),
    Pull(PullCommand),
    // TODO: maybe split between init local and setup repo
    // TODO: note to self: make first set named after hostname
    Init(InitCommand),
    LocalStatus(LocalStatusCommand),
    ChangeProfile(ChangeProfileCommand),
}

// TODO: macro?
impl Commands {
    fn execute(self, profile: MonjaProfile) {
        match self {
            Commands::Push(push_command) => push_command.execute(profile),
            Commands::Pull(pull_command) => pull_command.execute(profile),
            Commands::Init(init_command) => init_command.execute(profile),
            Commands::LocalStatus(local_status_command) => local_status_command.execute(profile),
            Commands::ChangeProfile(change_profile_command) => {
                change_profile_command.execute(profile)
            }
        }
    }
}

// TODO: macro?
#[derive(Args)]
struct PushCommand {}
impl PushCommand {
    fn execute(self, profile: MonjaProfile) {
        monja::push(&profile);
    }
}

#[derive(Args)]
struct PullCommand {}
impl PullCommand {
    fn execute(self, profile: MonjaProfile) {
        todo!()
    }
}

#[derive(Args)]
struct InitCommand {}
impl InitCommand {
    fn execute(self, profile: MonjaProfile) {
        todo!()
    }
}

#[derive(Args)]
struct LocalStatusCommand {}
impl LocalStatusCommand {
    fn execute(self, profile: MonjaProfile) {
        todo!()
    }
}

#[derive(Args)]
struct ChangeProfileCommand {}
impl ChangeProfileCommand {
    fn execute(self, profile: MonjaProfile) {
        todo!()
    }
}

fn main() {
    let mut profile_path =
        std::env::home_dir().expect("We got bigger problems if there's no home.");
    let local_root = profile_path.clone();
    profile_path.push(".monja-profile.toml");

    let profile_config = std::fs::read_to_string(profile_path).unwrap();
    let profile_config: MonjaProfileConfig = toml::from_str(&profile_config).unwrap();
    let profile = MonjaProfile {
        local_root: monja::AbsolutePath::new(local_root).unwrap(),
        repo_root: monja::AbsolutePath::new(profile_config.monja_dir).unwrap(),
        target_sets: profile_config.target_sets,
        new_file_set: profile_config.new_file_set,
    };

    let cli = Cli::parse();
    cli.command.execute(profile);
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
struct MonjaProfileConfig {
    pub monja_dir: PathBuf,
    pub target_sets: Vec<monja::SetName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_file_set: Option<monja::SetName>,
}
