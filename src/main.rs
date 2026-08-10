// #![deny(exported_private_dependencies)]
#![deny(clippy::unwrap_used)]
use std::fs;

use monja::{AbsolutePath, ExecutionOptions, MonjaProfile};

use anyhow::anyhow;
use clap::{Parser, Subcommand};

mod cli;
mod completions;

use cli::{
    files::FileArgs, init::InitCommand, new_set::NewSetCommand, packages::PackageArgs,
    profile::ProfileCommand, repo_dir::RepoDirCommand,
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    // also considering shoving everything except command into a flattened struct, but meh it fine for now
    #[command(flatten)]
    opts: ExecutionOptions,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[command(rename_all = "lower")]
enum Commands {
    /// Initializes a profile with some initial settings.
    ///
    /// A profile is created that uses a set named after the current hostname.
    /// The set also contains a sample `.monja-set.toml`.
    /// A `.monjaignore`` file is created in `$HOME` with some common defaults.
    Init(InitCommand),

    /// File management commands: push, pull, clean, put, transfer, setshortcut, status.
    File(FileArgs),

    /// Package management commands: add, remove, list, install.
    Package(PackageArgs),

    /// Creates a new set, with specified files, and adds it to the end of the profile.
    ///
    /// Note that this command ignores `.monjaignore` files.
    NewSet(NewSetCommand),

    /// Prints the repo's directory so that it can be piped into `cd`.
    RepoDir(RepoDirCommand),

    /// Prints the repo's directory so that it can be piped into `cd`.
    Profile(ProfileCommand),

    /// Prints the repo's directory so that it can be piped into `cd`.
    Completions(completions::CompletionsCommand),
}

impl Commands {
    fn execute(self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        match self {
            Commands::Init(_) => {
                panic!("Init command should have a separate invocation path.")
            }
            Commands::File(command) => command.execute(profile, opts),
            Commands::Package(command) => command.execute(profile, opts),
            Commands::NewSet(command) => command.execute(profile, opts),
            Commands::RepoDir(command) => command.execute(profile, opts),
            Commands::Profile(command) => command.execute(profile, opts),
            Commands::Completions(command) => command.execute(),
        }
    }
}

fn main() -> anyhow::Result<()> {
    completions::init();

    // goes first so that help and version commands can work before our code
    let cli = Cli::parse();

    let base = xdg::BaseDirectories::with_prefix("monja");

    let profile_config_path = base.place_config_file("monja-profile.toml")?;

    let local_root = std::env::home_dir().expect("We got bigger problems if there's no home.");
    let local_root = AbsolutePath::for_existing_path(&local_root)?;

    let data_root = base
        .get_data_home()
        .expect("We got bigger problems if there's no home.");
    fs::create_dir_all(&data_root)?;
    let data_root = AbsolutePath::for_existing_path(&data_root)?;

    // is a special case, since profile may not exist yet, etc.
    if let Commands::Init(init) = cli.command {
        return init.execute(cli.opts, profile_config_path, local_root, data_root, &base);
    }

    if !profile_config_path.is_file() {
        return Err(anyhow!(
            "monja profile does not exist. Run `monja init` to get started, or create the profile here: {}",
            profile_config_path.display()
        ));
    }

    let profile_config_path = AbsolutePath::for_existing_path(&profile_config_path)?;
    let profile_config = monja::MonjaProfileConfig::load(&profile_config_path)?;

    let profile = monja::MonjaProfile::from_config(profile_config, local_root, data_root)?;

    let dryrun = cli.opts.dry_run;
    cli.command.execute(profile, cli.opts)?;

    if dryrun {
        println!("Note that, due to being a dry-run, no changes were actually made.");
    }

    Ok(())
}
