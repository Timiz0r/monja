use std::{
    collections::HashSet,
    io::{BufRead, IsTerminal, Write},
    path::Path,
    process::{Command, Stdio},
};

use anyhow::anyhow;
use clap::{Args, Subcommand};
use monja::{ExecutionOptions, LocalFilePath, MonjaProfile};

mod clean;
mod pull;
mod push;
mod put;
mod set_shortcut;
mod status;
mod transfer;

use clean::CleanCommand;
use pull::PullCommand;
use push::PushCommand;
use put::PutCommand;
use set_shortcut::SetShortcutCommand;
use status::StatusCommand;
use transfer::TransferCommand;

#[derive(Args)]
pub struct FileArgs {
    #[command(subcommand)]
    command: FileCommands,
}
impl FileArgs {
    pub fn execute(self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        self.command.execute(profile, opts)
    }
}

#[derive(Subcommand)]
#[command(rename_all = "lower")]
enum FileCommands {
    /// Copies local files to the monja repo.
    ///
    /// This command uses information from the prior `monja file pull` to copy files into the right sets in the repo.
    /// It's important to note that this command may fail if files are removed from the repo that were previously pulled.
    /// As such, it is recommended to `monja file push` before doing such operations (like a `git pull`) to the repo.
    ///
    /// It will not copy files that have not been pulled.
    /// To copy such files to the repo, use `monja file put`.
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
    /// In the default mode, the sets of files pulled in the previous two `monja file pull`s are compared.
    /// Any file that was pulled in the older pull, but no longer pulled in the newer pull, gets removed.
    ///
    /// In the full mode, the current state of the repo is compared to the current state of local
    /// to determine which files should be removed locally.
    ///
    /// To prevent files from being cleaned, make sure they are covered by a `.monjaignore` file.
    Clean(CleanCommand),

    /// Puts local files into a set in the repo.
    ///
    /// Unlike `monja file push`, this works even if the file hasn't been pulled from the repo before.
    /// This is most commonly used to put files in the repo for the first time,
    /// or to recover from cases where `monja file push` is failing.
    ///
    /// Note that this command ignores `.monjaignore` files.
    Put(PutCommand),

    /// Transfers files from one set to another in the repo.
    ///
    /// The files must already be tracked by the source set.
    /// The destination set must be able to support each file (e.g. shortcut compatibility).
    ///
    /// Note that this command ignores `.monjaignore` files.
    #[command(name = "transfer")]
    Transfer(TransferCommand),

    /// Changes a set's shortcut path.
    ///
    /// The shortcut determines the common prefix stripped from local paths when storing files in the set.
    /// All existing files in the set must be representable under the new shortcut.
    SetShortcut(SetShortcutCommand),

    /// Prints detailed local status information.
    ///
    /// This command prints a few kinds of useful information, which can be filtered by additional args.
    /// If no filter is provided, everything will be shown.
    #[command(id = "status")]
    Status(StatusCommand),
}
impl FileCommands {
    fn execute(self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        match self {
            FileCommands::Push(command) => command.execute(profile, opts),
            FileCommands::Pull(command) => command.execute(profile, opts),
            FileCommands::Clean(command) => command.execute(profile, opts),
            FileCommands::Put(command) => command.execute(profile, opts),
            FileCommands::Transfer(command) => command.execute(profile, opts),
            FileCommands::SetShortcut(command) => command.execute(profile, opts),
            FileCommands::Status(command) => command.execute(profile, opts),
        }
    }
}

// commands that take local paths have a nocwd arg in order to be more easily used with fzf, etc
// where operations using external tools will preferably use paths relative to local_root
//
// shared with `cli::new_set`, which also collects local file paths the same three ways
// (positional args, stdin, and interactively via fzf) but isn't a `file` subcommand itself.
pub(crate) fn to_local_path(
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

pub(crate) fn to_local_paths(
    profile: &MonjaProfile,
    // impl trait allows us to use &vec instead of using an iterator that maps to &Path.
    // however, this is just for convenience, as we still use .collect instead of preallocating a vec, for Result reasons
    files: &[impl AsRef<Path>],
    cwd: &Path,
) -> anyhow::Result<Vec<LocalFilePath>> {
    let files: Result<Vec<LocalFilePath>, monja::LocalFilePathError> = files
        .iter()
        .map(|f| LocalFilePath::from(profile, f.as_ref(), cwd))
        .collect();
    Ok(files?)
}

pub(crate) fn read_paths_from_stdin(
    profile: &MonjaProfile,
    cwd: &Path,
) -> anyhow::Result<Vec<LocalFilePath>> {
    let stdin = std::io::stdin().lock();
    if stdin.is_terminal() {
        return Ok(Vec::new());
    }

    let mut ctr = 0;
    let paths: anyhow::Result<Vec<LocalFilePath>> = stdin
        .lines()
        .take_while(move |_| {
            ctr += 1;
            ctr < 100
        })
        .map(|s| {
            s.map_err(anyhow::Error::new).and_then(|s| {
                LocalFilePath::from(profile, s.as_ref(), cwd).map_err(anyhow::Error::new)
            })
        })
        .collect();
    if ctr >= 100 {
        // somewhat arbitrary, but better than mass copying, presumably
        Err(anyhow!(
            "There is a limit of 100 paths passed through stdin."
        ))
    } else {
        paths
    }
}

// arguably, this should be moved into operations. will decide later.
// as-is, this happens before any other validation, such as making sure a set exists
pub(crate) fn read_paths_interactively(
    profile: &MonjaProfile,
    files: impl Iterator<Item = LocalFilePath>,
) -> anyhow::Result<Vec<LocalFilePath>> {
    // could calculate capacity if necessary, but it's probably fine
    // we use a hashset for deduplication, since a file can be in multiple sets
    let mut deduped_files = HashSet::new();
    deduped_files.extend(files);

    let mut sorted_files = Vec::with_capacity(deduped_files.len());
    sorted_files.extend(deduped_files);
    sorted_files.sort();

    let mut child = Command::new("fzf")
        .args([
            &format!(
                "--preview=bat --binary=no-printing --style=default --color=always {}/{{}}",
                &profile.local_root
            ),
            "--multi",
            "--ansi",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("Failed to take stdin somehow when running fzf"))?;
    for file in sorted_files.into_iter() {
        stdin.write_all(file.as_os_str().as_encoded_bytes())?;
        stdin.write_all(b"\n")?;
    }
    std::mem::drop(stdin);

    let output = child.wait_with_output()?;
    if !output.status.success() {
        if output.status.code() == Some(130) {
            return Ok(Vec::new());
        }
        return Err(anyhow!(
            "Failed to run fzf: {:?}\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let files: anyhow::Result<Vec<LocalFilePath>> = output
        .stdout
        .lines()
        .map(|s| {
            s.map_err(anyhow::Error::new).and_then(|s| {
                LocalFilePath::from(profile, s.as_ref(), &profile.local_root)
                    .map_err(anyhow::Error::new)
            })
        })
        .collect();

    files
}
