use std::path::PathBuf;

use clap::Args;
use clap_complete::engine::ArgValueCandidates;
use monja::{AbsolutePath, ExecutionOptions, LocalFilePath, MonjaProfile, RepoName, SetName};

use crate::cli::files::{read_paths_from_stdin, read_paths_interactively, to_local_paths};
use crate::completions;

#[derive(Args)]
pub struct NewSetCommand {
    /// The set into which the files will be copied
    #[arg(long = "set")]
    new_set: String,

    /// The repo to create the set in.
    ///
    /// Only needed when the profile has several repos and no `default-repo` is configured.
    #[arg(long = "repo", add = ArgValueCandidates::new(completions::repo_names))]
    repo: Option<String>,

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

impl NewSetCommand {
    pub fn execute(self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        let cwd = match self.no_cwd {
            true => &profile.local_root,
            false => &AbsolutePath::for_existing_path(&std::env::current_dir()?)?,
        };
        let mut files = to_local_paths(&profile, &self.files, cwd)?;

        let mut stdin_files = read_paths_from_stdin(&profile, cwd)?;
        files.append(&mut stdin_files);

        // even though put is similar, there isn't really room to factor this code out.
        // most of the code is for combining multiple iterators, and put uses a different set.
        if self.interactive {
            let status = monja::local_status(
                &profile,
                LocalFilePath::from(&profile, cwd, &profile.local_root)?,
            )?;

            // old_files_after_last_pull is a special category that can contain duplicates of the other categories
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
                        .files_to_push // is the main difference to the code in put
                        .into_iter()
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

        let base = xdg::BaseDirectories::with_prefix("monja");
        let path = AbsolutePath::for_existing_path(&base.place_config_file("monja-profile.toml")?)?;
        let repo = self.repo.as_deref().map(RepoName::from);
        let result = monja::new_set(&profile, &opts, &path, files, SetName(self.new_set), repo)?;

        println!(
            "Successfully created new set `{}` in repo `{}` with the following files:",
            result.new_set, result.repo,
        );
        for file in result.files.into_iter() {
            println!("\t{}", file);
        }
        println!("The set has also been added to the profile.");

        Ok(())
    }
}
