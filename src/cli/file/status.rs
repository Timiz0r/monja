use std::path::PathBuf;

use clap::Args;
use monja::{ExecutionOptions, LocalFilePath, MonjaProfile, SetName};

use super::to_local_path;

#[derive(Args)]
pub struct StatusCommand {
    /// If set, the `location` argument provided will be relative to the local root, ignoring cwd.
    ///
    /// This is typically used when using external tools like `fzf` to select files.
    #[arg(long = "nocwd")]
    no_cwd: bool,

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

    /// Filter to files that would be pushed (if no error condition).
    #[arg(long)]
    old_files: bool,
}
impl StatusCommand {
    pub fn execute(&self, profile: MonjaProfile, _: ExecutionOptions) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let location = to_local_path(
            &profile,
            self.location.as_deref().unwrap_or("".as_ref()),
            &cwd,
            self.no_cwd,
        )?;
        print!(
            "Status of local files under {}\n\n",
            profile.local_root.join(&location).display()
        );

        let status = monja::local_status(&profile, location)?;

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

            if !status.untracked_files.is_empty() {
                for path in status.untracked_files.into_iter() {
                    println!("{}", path);
                }
            }
            println!();
        }

        if self.filter.as_ref().is_none_or(|f| f.old_files) {
            println!("Files removed from repo since last pull (also found in untracked):");

            if !status.old_files_after_last_pull.is_empty() {
                for path in status.old_files_after_last_pull.into_iter() {
                    println!("{}", path);
                }
            }
            println!();
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

            if !info.is_empty() {
                for (set_name, file_paths) in info {
                    println!("\tSet: {}", set_name);
                    for path in file_paths {
                        println!("\t\t{}", path);
                    }
                }
            }
            println!()
        }
    }
}
