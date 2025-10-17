// #![deny(exported_private_dependencies)]
#![deny(clippy::unwrap_used)]
use std::{
    fs,
    path::{Path, PathBuf},
};

use monja::{
    AbsolutePath, CleanMode, ExecutionOptions, InitSpec, LocalFilePath, MonjaProfile, SetName,
};

use anyhow::anyhow;
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
    /// Initializes a profile with some initial settings.
    ///
    /// A profile is created that uses a set named after the current hostname.
    /// The set also contains a sample `.monja-set.toml`.
    /// A `.monjaignore`` file is created in `$HOME` with some common defaults.
    Init(InitCommand),

    /// Copies local files to the monja repo.
    ///
    /// This command uses information from the prior `monja pull` to copy files into the right sets in the repo.
    /// It's important to note that this command may fail if files are removed from the repo that were previously pulled.
    /// As such, it is recommended to `monja push` before doing such operations (like a `git pull`) to the repo.
    ///
    /// It will not copy files that have not been pulled.
    /// To copy such files to the repo, use `monja put`.
    ///
    /// To keep files from being pushed, make sure they are covered by a `.monjaignore` file.
    Push(PushCommand),

    /// Copies files from the monja repo locally.
    ///
    /// The profile contains a list of sets to use, which are the folders in the root directory of the repo.
    /// These folders are evaluated in order. If a file is found in multiple targeted sets,
    /// then the latest set's file will be used.
    Pull(PullCommand),

    /// Removes local files that aren't handled by monja.
    ///
    /// In the default mode, the sets of files pulled in the previous two `monja pull`s are compared.
    /// Any file that was pulled in the older pull, but no longer pulled in the newer pull, gets removed.
    ///
    /// In the full mode, the current state of the repo is compared to the current state of local
    /// to determine which files should be removed locally.
    ///
    /// To prevent files from being cleaned, make sure they are covered by a `.monjaignore` file.
    Clean(CleanCommand),

    /// Puts local files into a set in the repo.
    ///
    /// Unlike `monja push`, this works even if the file hasn't been pulled from the repo before.
    /// This is most commonly used to put files in the repo for the first time,
    /// or to recover from cases where `monja push` is failing.
    ///
    /// Note that this command ignores `.monjaignore` files.
    Put(PutCommand),

    /// Prints detailed local status information.
    ///
    /// This command prints a few kinds of useful information, which can be filtered by additional args.
    /// If no filter is provided, everything will be shown.
    #[command(id = "status")]
    LocalStatus(StatusCommand),

    /// Prints the repo's directory so that it can be piped into `cd`.
    RepoDir(RepoDirCommand),
}
/*

    pub files_to_push: Vec<(repo::SetName, Vec<LocalFilePath>)>,
    pub files_with_missing_sets: Vec<(repo::SetName, Vec<LocalFilePath>)>,
    pub missing_files: Vec<(repo::SetName, Vec<LocalFilePath>)>,
    pub untracked_files: Vec<LocalFilePath>,
    pub old_files_after_last_pull: Vec<LocalFilePath>,
} */

// TODO: macro?
impl Commands {
    fn execute(self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        match self {
            Commands::Init(_) => {
                panic!("Init command should have a separate invocation path.")
            }
            Commands::Push(command) => command.execute(profile, opts),
            Commands::Pull(command) => command.execute(profile, opts),
            Commands::Clean(command) => command.execute(profile, opts),
            Commands::Put(command) => command.execute(profile, opts),
            Commands::LocalStatus(command) => command.execute(profile, opts),
            Commands::RepoDir(command) => command.execute(profile, opts),
        }
    }
}

#[derive(Args)]
struct InitCommand {}
impl InitCommand {
    fn execute(
        &self,
        opts: ExecutionOptions,
        profile_config_path: PathBuf,
        local_root: AbsolutePath,
        data_root: AbsolutePath,
        base: &xdg::BaseDirectories,
    ) -> anyhow::Result<()> {
        let repo_root = base.create_data_directory("repo")?;
        let repo_root = AbsolutePath::for_existing_path(&repo_root)?;
        let relative_repo_root = repo_root
            .strip_prefix(&local_root)
            .expect("Should naturally be a prefix")
            .to_path_buf();

        let machine = fs::read_to_string("/proc/sys/kernel/hostname")
            .expect("If doesn't exist, would prefer panic.")
            .trim()
            .to_string();

        let spec = InitSpec {
            profile_config_path,
            local_root,
            repo_root,
            data_root,
            relative_repo_root,
            initial_set_name: machine,
        };
        let result = monja::init(&opts, spec)?;

        match result.profile {
            Some(profile) => {
                println!("Initialization successful!");
                println!(
                    "Profile can be found at '{}'.",
                    result.profile_config_path.display()
                );
                println!("Repo can be found in '{}'.", profile.repo_root.display());
                println!(
                    "Set '{}' automatically created.",
                    profile.config.target_sets[0]
                );
            }
            None => println!("No changed made because dry-run."),
        };

        Ok(())
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
            let mut print_generic = false;
            if !files_with_missing_sets.is_empty() {
                print_generic = true;

                eprintln!("There are local files whose corresponding sets are missing.");

                eprintln!("Sets missing, as well as the files that currently require them:");
                for (set_name, file_paths) in files_with_missing_sets {
                    eprintln!("\tSet: {}", set_name);
                    for path in file_paths {
                        eprintln!("\t\t{:?}", path);
                    }
                }
            }
            if !missing_files.is_empty() {
                print_generic = true;

                eprintln!("There are local files missing from expected sets.");

                eprintln!("Files missing, as grouped under the sets they were expected to be in:");
                for (set_name, file_paths) in missing_files {
                    eprintln!("\tSet: {}", set_name);
                    for path in file_paths {
                        eprintln!("\t\t{:?}", path);
                    }
                }
            }

            if print_generic {
                eprint!(
                    "This happens due to changes being made in the repo without having yet pulled."
                );
                eprint!(
                    "It is recommended to `monja push` before doing a `git pull` or other repo modification."
                );
                eprintln!("To fix this, consider doing any of the the following:");

                eprintln!(
                    "\t* If there are no local changes that would get overwritten, use `monja pull`."
                );

                eprint!(
                    "\t* If the files should use a different set (such as the last specified in monja-profile.toml), "
                );
                eprint!(
                    "use some variation of `monja put --update-index` to specify that set and copy files to that set. "
                );
                eprintln!("Then, use `monja push` to push the rest of the files to the right set.");

                eprint!("\t* If the file is no longer needed, simply delete it. ");
                eprintln!(
                    "Then, use `monja push` to push these and the rest of the files to the right set."
                );

                eprintln!("\t* Manually merge local changes into the repo, then `monja pull`.");
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
                "Files pulled (including unchanged), as grouped under their corresponding sets:"
            );
            for (set_name, file_paths) in result.files_pulled.into_iter() {
                println!("\tSet: {}", set_name);
                for path in file_paths {
                    println!("\t\t'{:?}' -> '{:?}'", path.path_in_set, path.local_path);
                }
            }
        } else {
            println!("No files pulled.");
        }

        if !result.cleanable_files.is_empty() {
            println!("There are files present locally that are no longer pulled from the repo.");
            println!("If this is expected, do a `monja clean` to remove them.");
            println!(
                "If any are unexpected, copy them to a new set before performing `monja clean`."
            );

            for file_path in result.cleanable_files.into_iter() {
                println!("\t{:?}", file_path);
            }
        }

        Ok(())
    }
}

#[derive(Args)]
struct CleanCommand {
    /// If set, compares the full state of the repo against the local state,
    /// cleaning files that are not tracked in the repo.
    /// If not set, the previous two `monja pull`s are used to determine which files to clean.
    #[arg(long, short)]
    full: bool,
}
impl CleanCommand {
    fn execute(&self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        let mode = match self.full {
            true => CleanMode::Full,
            false => CleanMode::Index,
        };
        let clean_result = monja::clean(&profile, &opts, mode)?;

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
struct PutCommand {
    /// The set into which the files will be copied
    #[arg(long, id = "set")]
    owning_set: String,

    /// If set, the paths provided will be relative to the local root, ignoring cwd.
    ///
    /// This is typically used when using external tools like `fzf` to select files.
    #[arg(long)]
    nocwd: bool,

    /// If set, the local file index will be updated.
    ///
    /// This is typically used to fix issues that arise from `monja push` when files are missing
    /// from their expected locations in the repo (as determined by the last `monja pull`).
    #[arg(long)]
    update_index: bool,

    // TODO: also allow stdin
    /// The local files to copy. These will be combined with any newline-delimited files provided through stdin.
    files: Vec<PathBuf>,
}

impl PutCommand {
    fn execute(self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let files = to_local_paths(&profile, &self.files, &cwd, self.nocwd)?;

        let result = monja::put(
            &profile,
            &opts,
            files,
            SetName(self.owning_set),
            self.update_index,
        )?;

        println!(
            "Successfully changed the following files to use set `{}` (including copying them to the set):",
            result.owning_set
        );
        for file in result.files.into_iter() {
            println!("\t{:?}", file);
        }

        if !result.set_is_targeted {
            println!(
                "Note that set `{}` isn't targeted by the current profile, so it will not be eligible to be copied by `monja pull`.",
                result.owning_set
            );
        }

        if !result.files_in_later_sets.is_empty() {
            println!(
                "There were some files put into set `{0}` that, because they are also in sets later than `{0}`, wouldn't be copied by `monja pull`.",
                result.owning_set
            );
            for (path, set_names) in result.files_in_later_sets.into_iter() {
                println!("\t{:?}", path);
                for set_name in set_names.into_iter() {
                    println!("\t\t{}", set_name);
                }
            }
        }

        if !result.untracked_files.is_empty() {
            println!(
                "There were some files put into set `{}` that aren't in any of the sets used by the current profile.",
                result.owning_set
            );
            for file in result.untracked_files.into_iter() {
                println!("\t{:?}", file);
            }
        }

        Ok(())
    }
}

#[derive(Args)]
struct StatusCommand {
    /// If set, the `location` argument provided will be relative to the local root, ignoring cwd.
    ///
    /// This is typically used when using external tools like `fzf` to select files.
    #[arg(long)]
    nocwd: bool,

    /// The local location for which to view status.
    location: Option<PathBuf>,

    #[command(flatten)]
    filter: Option<StatusFilter>,
}

#[derive(Args)]
#[group(required = false, multiple = true)]
struct StatusFilter {
    /// Filter to files that are untracked by monja -- meaning they are not in any set targeted in the profile.
    #[arg(long)]
    untracked: bool,

    /// Filter to files, previously pulled, whose set at the time of the pull is currently missing.
    #[arg(long)]
    sets_missing: bool,

    /// Filter to files, previously pulled, that are no longer in the set they were previously pulled from.
    #[arg(long)]
    files_missing: bool,

    /// Filter to files that would be pushed (if no error condition).
    #[arg(long)]
    to_push: bool,
}
impl StatusCommand {
    fn execute(&self, profile: MonjaProfile, _: ExecutionOptions) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let location = to_local_path(
            &profile,
            self.location.as_deref().unwrap_or(".".as_ref()),
            &cwd,
            self.nocwd,
        )?;
        let status = monja::local_status(&profile, location)?;

        // TODO: revisit passing this to local_status
        // will probably pass cwd-rooted files for put command

        if self.filter.as_ref().is_none_or(|f| f.sets_missing) {
            print(
                "Sets missing, as well as the files that currently require them:",
                status.files_with_missing_sets,
            );
        }

        if self.filter.as_ref().is_none_or(|f| f.files_missing) {
            print(
                "Files missing, as grouped under the sets they were expected to be in:",
                status.missing_files,
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
            for path in status.old_files_after_last_pull.into_iter() {
                println!("{:?}", path);
            }
        }

        if self.filter.as_ref().is_none_or(|f| f.to_push) {
            print(
                "Files to push (including unchanged), as grouped under their corresponding sets:",
                status.files_to_push,
            );
        }

        return Ok(());

        fn print(message: &str, info: Vec<(SetName, Vec<LocalFilePath>)>) {
            println!("{}", message);
            for (set_name, file_paths) in info {
                println!("\tSet: {}", set_name);
                for path in file_paths {
                    println!("\t\t{:?}", path);
                }
            }
        }
    }
}

#[derive(Args)]
struct RepoDirCommand {}
impl RepoDirCommand {
    fn execute(&self, profile: MonjaProfile, _opts: ExecutionOptions) -> anyhow::Result<()> {
        println!("{}", profile.repo_root.display());

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    // goes first so that help and version commands can work before our code
    let cli = Cli::parse();

    let base = xdg::BaseDirectories::with_prefix("monja");

    let profile_config_path = base.place_config_file("monja-profile.toml")?;

    let local_root = std::env::home_dir().expect("We got bigger problems if there's no home.");
    let local_root = AbsolutePath::for_existing_path(&local_root)?;

    let data_root = base
        .get_data_home()
        .expect("We got bigger problems if there's no home.");
    fs::create_dir(&data_root)?;
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

    cli.command.execute(profile, cli.opts)
}

// commands that take local paths have a nocwd arg in order to be more easily used with fzf, etc
// where operations using external tools will preferably use paths relative to local_root
fn to_local_path(
    profile: &MonjaProfile,
    path: &Path,
    cwd: &Path,
    no_cwd: bool,
) -> anyhow::Result<LocalFilePath> {
    let cwd = match no_cwd {
        true => &profile.local_root,
        false => cwd,
    };
    Ok(LocalFilePath::from(profile, path, cwd)?)
}

fn to_local_paths(
    profile: &MonjaProfile,
    // impl trait allows us to use &vec instead of using an iterator that maps to &Path.
    // however, this is just for convenience, as we still use .collect instead of preallocating a vec, for Result reasons
    files: &[impl AsRef<Path>],
    cwd: &Path,
    no_cwd: bool,
) -> anyhow::Result<Vec<LocalFilePath>> {
    let cwd = match no_cwd {
        true => &profile.local_root,
        false => cwd,
    };
    let files: Result<Vec<LocalFilePath>, monja::LocalFilePathError> = files
        .iter()
        .map(|f| LocalFilePath::from(profile, f.as_ref(), cwd))
        .collect();
    Ok(files?)
}
