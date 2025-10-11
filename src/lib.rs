// #![deny(exported_private_dependencies)]
#![deny(clippy::unwrap_used)]
use std::{
    collections::{HashMap, HashSet},
    ffi::{OsStr, OsString},
    io::Write,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::LazyLock,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use repo::SetConfig;
pub use repo::SetName;
use repo::SetShortcut;

pub type LocalStateInitializationError = local::StateInitializationError;
//file index error is internal implementation detail

pub use repo::SetConfigError;
pub type RepoStateInitializationError = repo::StateInitializationError;
pub use repo::SetShortcutError;

pub(crate) mod local;
pub(crate) mod repo;

#[derive(Debug)]
pub struct AbsolutePath {
    path: PathBuf,
}
impl AbsolutePath {
    pub fn for_existing_path(path: &Path) -> Result<AbsolutePath, std::io::Error> {
        std::fs::canonicalize(path).map(|path| AbsolutePath { path })
    }

    // could implement Into, but won't implement From because this is fallible and meant to use for_existing_path
    // could implement TryFrom, though, instead of naming it for_existing_path,
    // but we'd still want this because it doesn't copy
    pub fn into_path_buf(self) -> PathBuf {
        self.path
    }
}
impl AsRef<Path> for AbsolutePath {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

// the original types use private dependency RelativePathBuf, so we add these types to get around it
#[derive(Debug)]
pub struct LocalFilePath(PathBuf);
impl From<LocalFilePath> for PathBuf {
    fn from(value: LocalFilePath) -> Self {
        value.0
    }
}
impl From<local::FilePath> for LocalFilePath {
    fn from(value: local::FilePath) -> Self {
        LocalFilePath(value.into_relative_path_buf().to_path(""))
    }
}
impl AsRef<Path> for LocalFilePath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

#[derive(Debug)]
pub struct RepoFilePath {
    pub path_in_set: PathBuf,
    pub local_path: PathBuf,
}
impl From<repo::FilePath> for RepoFilePath {
    fn from(value: repo::FilePath) -> Self {
        RepoFilePath {
            path_in_set: value.path_in_set.to_path(""),
            local_path: value.local_path.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MonjaProfileConfig {
    pub monja_dir: PathBuf,
    pub target_sets: Vec<SetName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_file_set: Option<SetName>,
}
impl MonjaProfileConfig {
    // we take a path to config file, not folder, since the profile could be one located in the repo, pointed to by local
    pub fn load(config_path: &AbsolutePath) -> Result<MonjaProfileConfig, MonjaProfileConfigError> {
        let config = std::fs::read(config_path.as_ref())?;

        Ok(toml::from_slice(&config)?)
    }

    pub fn save(&self, config_path: &AbsolutePath) -> Result<(), MonjaProfileConfigError> {
        Ok(std::fs::write(
            config_path.as_ref(),
            toml::to_string(&self)?,
        )?)
    }

    pub fn into_config(
        self,
        local_root: AbsolutePath,
    ) -> Result<MonjaProfile, MonjaProfileConfigError> {
        Ok(MonjaProfile {
            local_root,
            repo_root: AbsolutePath::for_existing_path(&self.monja_dir)?,
            config: self,
        })
    }
}

#[derive(Error, Debug)]
pub enum MonjaProfileConfigError {
    #[error("Unable to deserialize .monja-profile.toml.")]
    Deserialization(#[from] toml::de::Error),
    #[error("Unable to serialize .monja-profile.toml.")]
    Serialization(#[from] toml::ser::Error),
    #[error("Unable to save/load .monja-profile.toml.")]
    Io(#[from] std::io::Error),
}

#[derive(Debug)]
pub struct MonjaProfile {
    pub local_root: AbsolutePath,
    pub repo_root: AbsolutePath,

    pub config: MonjaProfileConfig,
}

#[derive(Error, Debug)]
pub enum PushError {
    #[error("Unable to initialize repo state.")]
    RepoStateInitialization(Vec<repo::StateInitializationError>),
    #[error("Unable to initialize local state.")]
    LocalStateInitialization(#[from] local::StateInitializationError),
    #[error("The local index and repo were found to be out of sync.")]
    Consistency {
        missing_sets: Vec<(repo::SetName, Vec<LocalFilePath>)>,
        missing_files: Vec<(repo::SetName, Vec<LocalFilePath>)>,
    },
    #[error("Failed to copy files via rsync.")]
    Rsync(#[source] std::io::Error),
}
#[derive(Debug)]
pub struct PushSuccess {
    pub files_pushed: Vec<(repo::SetName, Vec<LocalFilePath>)>,
}

pub fn push(profile: &MonjaProfile) -> Result<PushSuccess, PushError> {
    let repo = repo::initialize_full_state(profile).map_err(PushError::RepoStateInitialization)?;
    let local_state = local::retrieve_state(profile, &repo)?;

    if !local_state.missing_sets.is_empty() || !local_state.missing_files.is_empty() {
        let missing_sets = convert_path_result(
            local_state.missing_sets.len(),
            local_state.missing_sets.into_iter(),
        );
        let missing_files = convert_path_result(
            local_state.missing_files.len(),
            local_state.missing_files.into_iter(),
        );

        return Err(PushError::Consistency {
            missing_sets,
            missing_files,
        });
    }

    if !local_state.files_to_push.is_empty() {
        let mut repo = repo;
        for (set_name, files) in local_state.files_to_push.iter() {
            let set = repo
                .sets
                .remove(set_name)
                .expect("Already checked for missing sets.");

            // lets say set shortcut is foo/bar and file baz
            // transfer looks something like this: /home/xx/foo/bar/baz -> /monja/set/baz
            // here, the source is /home/xx/foo/bar/, dest is /monja/set/, and file is baz
            // incidentally, local::FilePath is foo/bar/baz
            rsync(
                set.shortcut.to_path(profile.local_root.as_ref()).as_path(),
                set.root.as_ref(),
                files
                    .iter()
                    .map(|local_path| set.shortcut.relative(local_path.as_ref()).to_path("")),
            )
            .map_err(PushError::Rsync)?;
        }
    }

    let files_to_push = convert_path_result(
        local_state.files_to_push.len(),
        local_state.files_to_push.into_iter(),
    );
    Ok(PushSuccess {
        files_pushed: files_to_push,
    })
}

#[derive(Error, Debug)]
pub enum PullError {
    #[error("Unable to initialize repo state.")]
    RepoStateInitialization(Vec<repo::StateInitializationError>),
    #[error("Sets needed by the profile are missing from the repo.")]
    MissingSets(Vec<repo::SetName>),
    #[error("Failed to copy files via rsync.")]
    Rsync(#[source] std::io::Error),
    #[error("Unable to read .monja-index.toml.")]
    // data structure is internal implementation detail, so just go with this.
    FileIndex,
}

#[derive(Debug)]
pub struct PullSuccess {
    pub files_pulled: Vec<(SetName, Vec<RepoFilePath>)>,
}

pub fn pull(profile: &MonjaProfile) -> Result<PullSuccess, PullError> {
    // the code ends up being the cleanest when files takes ownership of its data from repo,
    // since that data becomes part of the result.
    // in order to take ownership, we .remove() them (from sets).
    // if we instead used get(), we'd have to do a lot of cloning.
    // the only problem is we partial move the set, so it's not like we can store it anywhere directly.
    // instead, we just move out the rest of the set info we need, at the cost of a small hashmap.
    struct SetInfo {
        root: AbsolutePath,
        shortcut: SetShortcut,
    }
    let mut set_info = HashMap::with_capacity(profile.config.target_sets.len());

    let mut repo =
        repo::initialize_full_state(profile).map_err(PullError::RepoStateInitialization)?;
    // we first need a map on local path in order to pick the set associated with the file.
    // rsync, however, needs to be run per-set, so we'll group them later.
    let mut files: HashMap<local::FilePath, repo::File> = HashMap::new();

    let mut missing_sets = Vec::new();
    for set_name in profile.config.target_sets.iter() {
        if !repo.sets.contains_key(set_name) {
            missing_sets.push(set_name.clone());
            continue;
        };

        // if we find a missing set, save us the trouble of handling files
        if !missing_sets.is_empty() {
            continue;
        }

        let set = repo
            .sets
            .remove(set_name)
            .expect("We verified it existed where we aggregate missing sets.");
        set_info.insert(
            set_name,
            SetInfo {
                root: set.root,
                shortcut: set.shortcut,
            },
        );

        for (local_path, local_file) in set.locally_mapped_files.into_iter() {
            files.insert(local_path, local_file);
        }
    }
    // since we removed from the sets to get ownership of them, we want to move sets to ensure it doesn't get used.
    std::mem::drop(repo.sets);

    if !missing_sets.is_empty() {
        return Err(PullError::MissingSets(missing_sets));
    }

    let mut files_to_pull = HashMap::with_capacity(set_info.len());
    let mut index_files = HashMap::with_capacity(files.len());
    for (local_path, repo_file) in files.into_iter() {
        files_to_pull
            .entry(repo_file.owning_set.clone())
            .or_insert_with(Vec::new)
            .push(repo_file.path);

        index_files.insert(local_path, repo_file.owning_set);
    }

    for set_name in profile.config.target_sets.iter() {
        let set = set_info
            .get(set_name)
            .expect("Already checked for missing sets.");
        let file_paths = files_to_pull
            .get(set_name)
            .expect("Already checked for missing sets.");

        // lets say set shortcut is foo/bar and file baz
        // transfer looks something like this: /monja/set/baz -> /home/xx/foo/bar/baz
        // here, the source is /monja/set/, dest is /home/xx/foo/bar/, and file is baz
        // incidentally, local::FilePath is foo/bar/baz

        rsync(
            set.root.as_ref(),
            &set.shortcut.to_path(profile.local_root.as_ref()),
            file_paths.iter().map(|p| p.path_in_set.to_path("")),
        )
        .map_err(PullError::Rsync)?;
    }

    // TODO: what if rsync failed and we don't update index even though some copies happened?
    let index = local::FileIndex::new(index_files);
    index
        .save(&profile.local_root)
        .map_err(|_| PullError::FileIndex)?;

    let files_to_pull = convert_path_result(files_to_pull.len(), files_to_pull.into_iter());
    Ok(PullSuccess {
        files_pulled: files_to_pull,
    })
}

pub fn local_status(_profile: &MonjaProfile) {
    todo!()
}

// keeping as io result because basically everything is io result
fn rsync(source: &Path, dest: &Path, files: impl Iterator<Item = PathBuf>) -> std::io::Result<()> {
    // we use checksum mainly because, in integration tests, some files have same size and modified time
    // this could hypothetically happen in practice, so checksum is perhaps good.
    // note that file sizes still get compared before checksum, so most cases will still be fast.
    let mut child = Command::new("rsync")
        .args([
            "-av".as_ref(),
            "--files-from=-".as_ref(),
            "--checksum".as_ref(),
            "--mkpath".as_ref(),
            source.as_os_str(),
            // works with mkpath to ensure the dir is properly created if needed
            dest.join("").as_os_str(),
        ])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    {
        let mut stdin = child.stdin.take().expect("Added above");
        for file in files {
            // avoiding the fallible conversion to string
            stdin.write_all(file.as_os_str().as_bytes())?;
            stdin.write_all(b"\n")?;
        }
        // dropping sends eof
    }

    let status = child.wait_with_output()?;
    println!("Finished rsync with status {}", status.status);
    // TODO: would be nice to return these instead. would return both for success and failure.
    std::io::stdout().write_all(&status.stdout)?;
    std::io::stderr().write_all(&status.stderr)?;

    match status.status.success() {
        true => Ok(()),
        false => Err(std::io::Error::other("Unsuccessful status code for rsync.")),
    }
}

// not actually sure this is the best way, but it probably works
// and we can just test on windows if we ever support it
static MONJA_REPO_FILES: LazyLock<HashSet<OsString>> = LazyLock::new(|| {
    HashSet::from([
        OsString::from(".monja-set.toml"),
        OsString::from(".monja-dir.toml"),
    ])
});
static MONJA_LOCAL_FILES: LazyLock<HashSet<OsString>> = LazyLock::new(|| {
    HashSet::from([
        OsString::from(".monja-profile.toml"),
        OsString::from(".monja-index.toml"),
        OsString::from(".monjaignore.toml"),
    ])
});
pub fn is_monja_repo_file(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|f: &OsStr| MONJA_REPO_FILES.contains(f))
}
pub fn is_monja_local_file(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|f: &OsStr| MONJA_LOCAL_FILES.contains(f))
}

// want to keep local/repo::File internal, so gonna bite the bullet on allocating another vector.
// this is mainly to avoid exporting RelativePath(Buf).
fn convert_path_result<Orig, Next>(
    length: usize,
    source: impl Iterator<Item = (repo::SetName, Vec<Orig>)>,
) -> Vec<(repo::SetName, Vec<Next>)>
where
    Orig: Into<Next>,
{
    let mut result = Vec::with_capacity(length);
    result.extend(
        source
            .into_iter()
            .map(|(set, file_paths)| (set, file_paths.into_iter().map(|p| p.into()).collect())),
    );

    result
}
