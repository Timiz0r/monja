use std::path::PathBuf;

use clap::Args;
use clap_complete::engine::ArgValueCandidates;
use monja::{AbsolutePath, ExecutionOptions, LocalFilePath, MonjaProfile, SetName};

use crate::completions;

use super::{read_paths_from_stdin, read_paths_interactively, to_local_paths};

#[derive(Args)]
pub struct TransferCommand {
    /// The set to move files from
    #[arg(long = "from", add = ArgValueCandidates::new(completions::set_names))]
    source_set: String,

    /// The set to move files to
    #[arg(long = "to", add = ArgValueCandidates::new(completions::set_names))]
    dest_set: String,

    /// If set, the paths provided will be relative to the local root, ignoring cwd.
    ///
    /// This is typically used when using external tools like `fzf` to select files.
    #[arg(long = "nocwd")]
    no_cwd: bool,

    /// Uses `fzf` to select local files to transfer.
    #[arg(long, short)]
    interactive: bool,

    /// The local files to transfer.
    ///
    /// These will be combined with any newline-delimited files provided through stdin.
    /// These will also be combined with files provided via `--interactive`.
    ///
    /// A limit of 100 paths may be passed through stdin to prevent accidental mass copying.
    #[arg(last = true)]
    files: Vec<PathBuf>,
}

impl TransferCommand {
    pub fn execute(self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        let cwd = match self.no_cwd {
            true => &profile.local_root,
            false => &AbsolutePath::for_existing_path(&std::env::current_dir()?)?,
        };
        let source_set = SetName(self.source_set);
        let dest_set = SetName(self.dest_set);

        let mut files = to_local_paths(&profile, &self.files, cwd)?;

        let mut stdin_files = read_paths_from_stdin(&profile, cwd)?;
        files.append(&mut stdin_files);

        if self.interactive {
            let status = monja::local_status(
                &profile,
                LocalFilePath::from(&profile, &profile.local_root, cwd)?,
            )?;

            let interactive_files = status
                .files_to_push
                .into_iter()
                .filter(|(set_name, _)| set_name == &source_set)
                .flat_map(|(_, files)| files);
            let mut interactive_files = read_paths_interactively(&profile, interactive_files)?;
            files.append(&mut interactive_files);
        }

        if files.is_empty() {
            println!("No files selected.");
            return Ok(());
        }

        let result = monja::transfer(&profile, &opts, files, source_set, dest_set)?;

        println!(
            "Successfully transferred the following files from set `{}` to set `{}`:",
            result.source_set, result.dest_set
        );
        for file in result.files.into_iter() {
            println!("\t{}", file);
        }

        Ok(())
    }
}
