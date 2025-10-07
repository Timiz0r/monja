use std::{
    collections::HashMap,
    io::Write,
    ops::Deref,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub(crate) mod local;
pub(crate) mod repo;

pub use repo::SetName;
use serde::{Deserialize, Serialize};

pub struct AbsolutePath {
    path: PathBuf,
}
impl AbsolutePath {
    pub fn from_path(path: PathBuf) -> Result<AbsolutePath, std::io::Error> {
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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MonjaProfileConfig {
    pub monja_dir: PathBuf,
    pub target_sets: Vec<SetName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_file_set: Option<SetName>,
}

pub struct MonjaProfile {
    pub local_root: AbsolutePath,
    pub repo_root: AbsolutePath,

    pub target_sets: Vec<repo::SetName>,
    pub new_file_set: Option<repo::SetName>,
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
            //TODO: see https://doc.rust-lang.org/stable/std/error/index.html#common-message-styles
            .expect("Already checked for missing sets.");

        // TODO: test that (attacker) modification of the index in order to traverse higher directories isnt possible
        //      since we could always move from rsync in the future and miss this
        // note that because we use rsync with a source folder specified, we cant escape to a higher level,
        // which mitigates potential directory traversal attacks.

        // lets say set shortcut is foo/bar and file baz
        // transfer looks something like this: /home/xx/foo/bar/baz -> /monja/set/baz
        // here, the source is /home/xx/foo/bar/, dest is /monja/set/, and file is baz
        // incidentally, local::FilePath is foo/bar/baz
        rsync(
            set.shortcut().to_path(&profile.local_root),
            set.root(),
            files
                .iter()
                // TODO: could move this logic to repo module, since it knows both local and repo paths, plus how to map
                .map(|local_path| {
                    set.shortcut()
                        .relative(local_path.relative_path())
                        .into_string()
                }),
        )
        .unwrap();
    }
}

pub fn pull(profile: &MonjaProfile) {
    let repo = repo::initialize_full_state(&profile.repo_root).unwrap();
    // we first need a map on local path in order to pick the set associated with the file.
    // rsync, however, needs to be run per-set, so we'll group them later.
    let mut files: HashMap<&local::FilePath, &repo::File> = HashMap::new();

    let mut missing_sets = vec![];
    for set in profile.target_sets.iter() {
        let Some(set) = repo.get_set(set) else {
            missing_sets.push(set);
            continue;
        };

        // if we find a missing set, save us the trouble of handling files
        if !missing_sets.is_empty() {
            continue;
        }

        for file in set.files() {
            files.insert(file.path.local_path(), file);
        }
    }

    if !missing_sets.is_empty() {
        eprintln!(
            "Sets needed by the profile are missing from the repo: {:?}",
            missing_sets
        );
        eprintln!("Verify that the right set of sets in '.monja-profile.toml' are present.");
        return;
    }

    let mut files_to_pull = HashMap::with_capacity(repo.set_count());
    for repo_file in files.values() {
        files_to_pull
            .entry(&repo_file.owning_set)
            .or_insert_with(Vec::new)
            .push(&repo_file.path);
    }

    println!("Files to be pulled, as grouped under their corresponding sets:");
    for (set_name, file_paths) in files_to_pull.iter() {
        eprintln!("\tSet: {}", set_name);
        for path in file_paths {
            eprintln!(
                "\t\t'{}' -> '{}'",
                path.path_in_set(),
                path.local_path().relative_path()
            );
        }
    }

    for (set_name, file_paths) in files_to_pull.iter() {
        let set = repo
            .get_set(set_name)
            .expect("Already checked for missing sets.");

        // lets say set shortcut is foo/bar and file baz
        // transfer looks something like this: /monja/set/baz -> /home/xx/foo/bar/baz
        // here, the source is /monja/set/, dest is /home/xx/foo/bar/, and file is baz
        // incidentally, local::FilePath is foo/bar/baz
        rsync(
            set.root(),
            set.shortcut().to_path(&profile.local_root),
            file_paths.iter().map(|p| p.path_in_set().as_str()),
        )
        .unwrap();
    }

    let mut index_files = HashMap::with_capacity(files.len());
    index_files.extend(
        files
            .into_iter()
            .map(|(local_path, repo_file)| (local_path.clone(), repo_file.owning_set.clone())),
    );
    let index = local::FileIndex::new(index_files);
    index.save(&profile.repo_root);
}

pub fn local_status(profile: &MonjaProfile) {
    todo!()
}

fn rsync<Src, Dest, Files>(source: Src, dest: Dest, files: Files) -> std::io::Result<()>
where
    Src: AsRef<Path>,
    Dest: AsRef<Path>,
    Files: Iterator<Item: AsRef<Path>>,
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
