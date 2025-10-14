// #![deny(exported_private_dependencies)]
#![deny(clippy::unwrap_used)]
use std::path::{Path, PathBuf};

use monja::{
    AbsolutePath, CleanMode, ExecutionOptions, LocalFilePath, MonjaProfile, SetName, clean,
    operation::status::local_status,
};

use clap::{Args, Parser, Subcommand, command};

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
enum Commands {
    Push(PushCommand),
    Pull(PullCommand),
    Clean(CleanCommand),
    Init(InitCommand),
    LocalStatus(StatusCommand),
    ChangeProfile(ChangeProfileCommand),
}

// TODO: macro?
impl Commands {
    fn execute(&self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        match self {
            Commands::Push(command) => command.execute(profile, opts),
            Commands::Pull(command) => command.execute(profile, opts),
            Commands::Clean(command) => command.execute(profile, opts),
            Commands::Init(command) => command.execute(profile, opts),
            Commands::LocalStatus(command) => command.execute(profile, opts),
            Commands::ChangeProfile(command) => command.execute(profile, opts),
        }
    }
}

#[derive(Args)]
struct PushCommand {}
impl PushCommand {
    fn execute(&self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        let result = monja::push(&profile, &opts);

        // want better logging for this
        if let Err(monja::PushError::Consistency {
            files_with_missing_sets,
            missing_files,
        }) = result
        {
            if !files_with_missing_sets.is_empty() {
                // TODO: better recovery mechanisms
                // easiest would be to select the last set, after which the user can head to the repo to figure it out.
                eprint!("There are local files whose corresponding sets are missing. ");
                eprintln!(
                    "To fix this, manually merge local changes into the repo, then pull the repo."
                );

                eprintln!("Sets missing, as well as the files that currently require them:");
                for (set_name, file_paths) in files_with_missing_sets {
                    eprintln!("\tSet: {}", set_name);
                    for path in file_paths {
                        eprintln!("\t\t{:?}", path);
                    }
                }
            }
            if !missing_files.is_empty() {
                // TODO: better recovery mechanisms
                // easiest would be to recreate the file in the set or pick the last set
                eprint!("There are local files missing from expected sets.");
                eprintln!(
                    "To fix this, manually merge local changes into the repo, then pull the repo."
                );

                eprintln!("Files missing, as grouped under the sets they were expected to be in:");
                for (set_name, file_paths) in missing_files {
                    eprintln!("\tSet: {}", set_name);
                    for path in file_paths {
                        eprintln!("\t\t{:?}", path);
                    }
                }
            }
            // probably something better to use, but we don't want to double log with the below `result?`.
            return Err(anyhow::Error::msg("Failed to push."));
        }

        // log rest of errors like this because lazy
        let result = result?;

        if !result.files_pushed.is_empty() {
            println!(
                "Files pushed (including unchanged), as grouped under their corresponding sets:"
            );
            for (set_name, file_paths) in result.files_pushed.iter() {
                eprintln!("\tSet: {}", set_name);
                for path in file_paths {
                    eprintln!("\t\t{:?}", path);
                }
            }
        } else {
            println!("No files pushed.");
        }

        Ok(())
    }
}

#[derive(Args)]
struct PullCommand {}
impl PullCommand {
    fn execute(&self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        let result = monja::pull(&profile, &opts);

        if let Err(monja::PullError::MissingSets(missing_sets)) = result {
            eprintln!(
                "Sets needed by the profile are missing from the repo: {:?}",
                missing_sets
            );
            eprintln!("Verify that the right set of sets in 'monja-profile.toml' are present.");
            // probably something better to use, but we don't want to double log with the below `result?`.
            return Err(anyhow::Error::msg("Failed to pull."));
        }

        let result = result?;

        if !result.files_pulled.is_empty() {
            println!(
                "Files to be pulled (including unchanged), as grouped under their corresponding sets:"
            );
            for (set_name, file_paths) in result.files_pulled.into_iter() {
                eprintln!("\tSet: {}", set_name);
                for path in file_paths {
                    eprintln!("\t\t'{:?}' -> '{:?}'", path.path_in_set, path.local_path);
                }
            }
        } else {
            println!("No files pulled.");
        }

        Ok(())
    }
}

#[derive(Args)]
struct CleanCommand {
    // if false, will use index diff from last pull
    #[arg()]
    full: bool,
}
impl CleanCommand {
    fn execute(&self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        let mode = match self.full {
            true => CleanMode::Full,
            false => CleanMode::Index,
        };
        let clean_result = clean(&profile, &opts, mode)?;

        if !clean_result.files_cleaned.is_empty() {
            println!("Local files cleaned:");
            for path in clean_result.files_cleaned.into_iter() {
                println!("{:?}", path);
            }
        } else {
            println!("No local files cleaned.")
        }

        Ok(())
    }
}

#[derive(Args)]
struct InitCommand {}
impl InitCommand {
    fn execute(&self, _profile: MonjaProfile, _opts: ExecutionOptions) -> anyhow::Result<()> {
        todo!()
    }
}

#[derive(Args)]
struct StatusCommand {
    location: Option<PathBuf>,

    #[command(flatten)]
    filter: Option<StatusFilter>,
}

#[derive(Args)]
#[group(required = false, multiple = true)]
struct StatusFilter {
    #[arg(long)]
    untracked: bool,
    #[arg(long)]
    sets_missing: bool,
    #[arg(long)]
    files_missing: bool,
    #[arg(long)]
    to_push: bool,
}
impl StatusCommand {
    fn execute(&self, profile: MonjaProfile, _: ExecutionOptions) -> anyhow::Result<()> {
        let status = local_status(&profile)?;

        let location = self
            .location
            .as_deref()
            .unwrap_or(".".as_ref())
            .canonicalize()?;

        if self.filter.as_ref().is_none_or(|f| f.sets_missing) {
            print(
                "Sets missing, as well as the files that currently require them:",
                status.files_with_missing_sets,
                &location,
            );
        }

        if self.filter.as_ref().is_none_or(|f| f.files_missing) {
            print(
                "Files missing, as grouped under the sets they were expected to be in:",
                status.missing_files,
                &location,
            );
        }

        if self.filter.as_ref().is_none_or(|f| f.untracked) {
            println!("Untracked files:");
            for path in status.untracked_files.into_iter() {
                println!("{:?}", path);
            }
        }

        if self.filter.as_ref().is_none_or(|f| f.untracked) {
            println!("Files removed from repo since last pull (also found in untracked):");
            for path in status.old_files_since_last_pull.into_iter() {
                println!("{:?}", path);
            }
        }

        if self.filter.as_ref().is_none_or(|f| f.to_push) {
            print(
                "Files to push (including unchanged), as grouped under their corresponding sets:",
                status.files_to_push,
                &location,
            );
        }

        return Ok(());

        fn print(message: &str, info: Vec<(SetName, Vec<LocalFilePath>)>, location: &Path) {
            println!("{}", message);
            for (set_name, file_paths) in info {
                println!("\tSet: {}", set_name);
                for path in file_paths {
                    let abs = {
                        let this: &Path = &path;
                        this
                    }
                    .canonicalize();
                    if abs.is_ok_and(|p| p.starts_with(location)) {
                        println!("\t\t{:?}", path);
                    }
                }
            }
        }
    }
}

#[derive(Args)]
struct ChangeProfileCommand {}
impl ChangeProfileCommand {
    fn execute(&self, _profile: MonjaProfile, _opts: ExecutionOptions) -> anyhow::Result<()> {
        todo!()
    }
}

fn main() -> anyhow::Result<()> {
    let base = xdg::BaseDirectories::with_prefix("monja");

    let data_root = base
        .get_data_home()
        .expect("We got bigger problems if there's no home.");
    let data_root = AbsolutePath::for_existing_path(&data_root)?;

    let profile_config_path =
        AbsolutePath::for_existing_path(&base.place_config_file("monja-profile.toml")?)?;
    let profile_config = monja::MonjaProfileConfig::load(&profile_config_path)?;

    let local_root = std::env::home_dir().expect("We got bigger problems if there's no home.");
    let local_root = AbsolutePath::for_existing_path(&local_root)?;

    let profile = monja::MonjaProfile::from_config(profile_config, local_root, data_root)?;

    let cli = Cli::parse();
    cli.command.execute(profile, cli.opts)
}
