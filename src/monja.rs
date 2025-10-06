use std::{
    io::Write,
    ops::Deref,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};

pub(crate) mod local;
pub(crate) mod repo;

pub struct AbsolutePath {
    path: PathBuf,
}
impl AbsolutePath {
    pub fn new(path: PathBuf) -> Result<AbsolutePath, std::io::Error> {
        std::fs::canonicalize(path).map(|path| AbsolutePath { path })
    }
}
impl Deref for AbsolutePath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}
impl AsRef<Path> for AbsolutePath {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

pub struct MonjaProfile {
    pub local_root: AbsolutePath,
    pub repo_root: AbsolutePath,

    pub target_sets: Vec<repo::SetName>,
    pub new_file_set: repo::SetName,
}

// TODO: return result and less unwraps
pub fn push(profile: &MonjaProfile) {
    let repo = repo::initialize_full_state(&profile.repo_root).unwrap();
    let local_state = local::retrieve_state(profile, &repo);

    let mut cont = true;
    if !local_state.missing_sets.is_empty() {
        cont = false;

        // TODO: better recovery mechanisms
        // easiest would be to select the last set, after which the user can head to the repo to figure it out.
        eprint!("There are local files whose corresponding sets are missing. ");
        eprintln!("To fix this, manually merge local changes into the repo, then pull the repo.");

        eprintln!("Sets missing, as well as the files that currently require them:");
        for (set_name, file_paths) in local_state.missing_sets {
            eprintln!("\tSet: {}", set_name);
            for path in file_paths {
                eprintln!("\t\t{}", path.relative_path());
            }
        }
    }
    if !local_state.missing_files.is_empty() {
        cont = false;

        // TODO: better recovery mechanisms
        // easiest would be to recreate the file in the set or pick the last set
        eprint!("There are local files missing from expected sets.");
        eprintln!("To fix this, manually merge local changes into the repo, then pull the repo.");

        eprintln!("Files missing, as grouped under the sets they were expected to be in:");
        for (set_name, file_paths) in local_state.missing_files {
            eprintln!("\tSet: {}", set_name);
            for path in file_paths {
                eprintln!("\t\t{}", path.relative_path());
            }
        }
    }

    if !cont {
        return;
    }

    if local_state.files_to_push.is_empty() {
        println!("No files to be pushed.");
        return;
    };

    println!("Files to be pushed, as grouped under their corresponding sets:");
    for (set_name, file_paths) in local_state.files_to_push.iter() {
        eprintln!("\tSet: {}", set_name);
        for path in file_paths {
            eprintln!("\t\t{}", path.relative_path());
        }
    }

    for (set_name, files) in local_state.files_to_push.iter() {
        let set = repo
            .get_set(set_name)
            .expect("Already checked for missing sets.");

        rsync(
            profile.local_root.join(set.relative_root().to_path("")),
            set.absolute_root(),
            files
                .iter()
                // TODO: could move this logic to repo, since it knows both local and repo paths, plus how to map
                .map(|f| set.relative_root().relative(f.relative_path()).to_path("")),
        )
        .unwrap();
    }
}

pub fn local_status(profile: &MonjaProfile) {
    todo!()
}

fn rsync<Src, Dest, File, Files>(source: Src, dest: Dest, files: Files) -> std::io::Result<()>
where
    Src: AsRef<Path>,
    Dest: AsRef<Path>,
    File: AsRef<Path>,
    Files: Iterator<Item = File>,
{
    let mut child = Command::new("rsync")
        .args([
            "-av",
            "--files-from=-",
            source.as_ref().to_str().unwrap(),
            dest.as_ref().to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .spawn()?;

    {
        let mut stdin = child.stdin.take().unwrap();
        for file in files {
            writeln!(stdin, "{}", file.as_ref().to_str().unwrap())?;
        }
        // dropping sends eof
    }

    // TODO: realistically want to stream it, but going with easy one for now
    let status = child.wait_with_output()?;
    println!("Finished rsync with status {}", status.status);
    std::io::stdout().write_all(&status.stdout)?;
    std::io::stderr().write_all(&status.stderr)?;

    match status.status.success() {
        true => Ok(()),
        // TODO: dont really want io errors, so all this is just temp
        false => Err(std::io::Error::other("Failed")),
    }
}
