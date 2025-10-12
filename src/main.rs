// #![deny(exported_private_dependencies)]
#![deny(clippy::unwrap_used)]
use monja::{AbsolutePath, ExecutionOptions, MonjaProfile};

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
    // TODO: maybe split between init local and setup repo
    // TODO: note to self: make first set named after hostname
    Init(InitCommand),
    LocalStatus(LocalStatusCommand),
    ChangeProfile(ChangeProfileCommand),
}

// TODO: macro?
impl Commands {
    fn execute(&self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        match self {
            Commands::Push(push_command) => push_command.execute(profile, opts),
            Commands::Pull(pull_command) => pull_command.execute(profile, opts),
            Commands::Init(init_command) => init_command.execute(profile, opts),
            Commands::LocalStatus(local_status_command) => {
                local_status_command.execute(profile, opts)
            }
            Commands::ChangeProfile(change_profile_command) => {
                change_profile_command.execute(profile, opts)
            }
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
            println!("Files pushed, as grouped under their corresponding sets:");
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
            eprintln!("Verify that the right set of sets in '.monja-profile.toml' are present.");
            // probably something better to use, but we don't want to double log with the below `result?`.
            return Err(anyhow::Error::msg("Failed to pull."));
        }

        let result = result?;

        if !result.files_pulled.is_empty() {
            println!("Files to be pulled, as grouped under their corresponding sets:");
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
struct InitCommand {}
impl InitCommand {
    fn execute(&self, _profile: MonjaProfile, _opts: ExecutionOptions) -> anyhow::Result<()> {
        todo!()
    }
}

#[derive(Args)]
struct LocalStatusCommand {}
impl LocalStatusCommand {
    fn execute(&self, _profile: MonjaProfile, _opts: ExecutionOptions) -> anyhow::Result<()> {
        todo!()
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
    let mut profile_path =
        std::env::home_dir().expect("We got bigger problems if there's no home.");
    let local_root = AbsolutePath::for_existing_path(&profile_path)?;
    profile_path.push(".monja-profile.toml");
    let profile_path = AbsolutePath::for_existing_path(&profile_path)?;

    let profile = monja::MonjaProfile::from_config(
        monja::MonjaProfileConfig::load(&profile_path)?,
        local_root,
    )?;

    let cli = Cli::parse();
    cli.command.execute(profile, cli.opts)
}
