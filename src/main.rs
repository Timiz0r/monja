use clap::{Args, Parser, Subcommand, command};

use monja::{AbsolutePath, MonjaProfile};

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

// TODO: some things named root should probably be base?

fn main() {
    let mut profile_path =
        std::env::home_dir().expect("We got bigger problems if there's no home.");
    let local_root = AbsolutePath::from_path(profile_path.clone()).unwrap();
    profile_path.push(".monja-profile.toml");
    let profile_path = AbsolutePath::from_path(profile_path).unwrap();

    let profile = monja::MonjaProfileConfig::load(&profile_path).into_config(local_root);

    let cli = Cli::parse();
    cli.command.execute(profile);
}
