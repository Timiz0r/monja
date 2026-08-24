use std::path::PathBuf;

use clap::Args;
use clap_complete::engine::ArgValueCandidates;
use monja::{AbsolutePath, ExecutionOptions, LocalFilePath, MonjaProfile, SetName};

use crate::completions;

use super::{read_paths_from_stdin, read_paths_interactively, to_local_paths};

#[derive(Args)]
pub struct PutCommand {
    /// The set into which the files will be copied
    #[arg(long = "set", add = ArgValueCandidates::new(completions::set_names))]
    owning_set: String,

    /// If set, the paths provided will be relative to the local root, ignoring cwd.
    ///
    /// This is typically used when using external tools like `fzf` to select files.
    #[arg(long = "nocwd")]
    no_cwd: bool,

    /// Uses `fzf` to select local files to copy.
    #[arg(long, short)]
    interactive: bool,

    /// The local files to copy.
    ///
    /// These will be combined with any newline-delimited files provided through stdin.
    /// These will also be combined with files provided via `--interactive`.
    ///
    /// A limit of 100 paths may be passed through stdin to prevent accidental mass copying.
    #[arg(last = true)]
    files: Vec<PathBuf>,
}

impl PutCommand {
    pub fn execute(self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        let cwd = match self.no_cwd {
            true => &profile.local_root,
            false => &AbsolutePath::for_existing_path(&std::env::current_dir()?)?,
        };
        let owning_set = SetName(self.owning_set);

        let mut files = to_local_paths(&profile, &self.files, cwd)?;

        let mut stdin_files = read_paths_from_stdin(&profile, cwd)?;
        files.append(&mut stdin_files);

        if self.interactive {
            let status = monja::local_status(
                &profile,
                LocalFilePath::from(&profile, &profile.local_root, cwd)?,
            )?;

            // since files_to_push means the (targeted) set already has the file, we don't need to include them.
            // additionally, old_files_after_last_pull is a special category that can contain duplicates of the other categories
            let interactive_files = status
                .files_with_missing_sets
                .into_iter()
                .flat_map(|(_, files)| files)
                .chain(
                    status
                        .missing_files
                        .into_iter()
                        .flat_map(|(_, files)| files),
                )
                .chain(
                    status
                        .files_to_push
                        .into_iter()
                        .filter(|(set_name, _)| set_name != &owning_set)
                        .flat_map(|(_, files)| files),
                )
                .chain(status.untracked_files);
            let mut interactive_files = read_paths_interactively(&profile, interactive_files)?;
            files.append(&mut interactive_files);
        }

        // there could hypothetically be duplicates between these three sources, or even in a single source
        // let's just assume the user doesnt. and, for all i know, it'll work just fine.

        if files.is_empty() {
            // could consider it an error, but it's not a big deal that the user didn't provide anything
            println!("No files selected.");
            return Ok(());
        }

        let result = monja::put(&profile, &opts, files, owning_set)?;

        println!(
            "Successfully changed the following files to use set `{}` in repo `{}` (including copying them to the set):",
            result.owning_set, result.repo
        );
        for file in result.files.into_iter() {
            println!("\t{}", file);
        }

        if !result.set_is_targeted {
            println!(
                "Note that set `{}` isn't targeted by the current profile, so it will not be eligible to be copied by `monja file pull`.",
                result.owning_set
            );
        }

        if !result.files_in_later_sets.is_empty() {
            println!(
                "There were some files put into set `{0}` that, because they are also in sets later than `{0}`, wouldn't be copied by `monja file pull`.",
                result.owning_set
            );
            for (path, set_names) in result.files_in_later_sets.into_iter() {
                println!("\t{}", path);
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
                println!("\t{}", file);
            }
        }

        Ok(())
    }
}
