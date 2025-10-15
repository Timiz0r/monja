use std::{collections::HashMap, fmt::Display, fs, ops::Deref, path::PathBuf};

use relative_path::{RelativePath, RelativePathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use walkdir::WalkDir;

use crate::{AbsolutePath, MonjaProfile, local};

pub(crate) struct RepoState {
    pub sets: HashMap<SetName, Set>,
}

pub(crate) struct Set {
    pub _name: SetName,
    pub shortcut: SetShortcut,
    pub root: AbsolutePath,
    // directories: HashMap<ObjectPath, Directory>,
    pub locally_mapped_files: HashMap<local::FilePath, File>,
}

impl Set {
    pub(crate) fn tracks_file(&self, local_path: &local::FilePath) -> bool {
        self.locally_mapped_files.contains_key(local_path)
    }

    // returns PathBuf because AbsolutePath requires the file exist
    pub(crate) fn get_repo_absolute_path_for(&self, local_path: &local::FilePath) -> PathBuf {
        self.get_repo_relative_path_for(local_path)
            .to_path(&self.root)
    }

    pub(crate) fn get_repo_relative_path_for(
        &self,
        local_path: &local::FilePath,
    ) -> RelativePathBuf {
        self.shortcut.relative(local_path)
    }
}

pub(crate) struct FilePath {
    pub path_in_set: RelativePathBuf,
    pub local_path: local::FilePath,
}

impl FilePath {
    fn new(shortcut: &RelativePath, path_in_set: RelativePathBuf) -> FilePath {
        let mut local_path = RelativePathBuf::new();
        local_path.push(shortcut);
        local_path.push(&path_in_set);
        let local_path = local::FilePath::new(local_path);

        FilePath {
            path_in_set,
            local_path,
        }
    }
}

pub(crate) struct File {
    pub owning_set: SetName,
    pub path: FilePath,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct SetConfig {
    // used to be called root, but it was hard to disambiguate with other uses of the term
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<PathBuf>,
}

impl SetConfig {
    pub fn load(
        profile: &crate::MonjaProfile,
        set_name: &SetName,
    ) -> Result<SetConfig, SetConfigError> {
        let config_path = profile.repo_root.join(set_name).join(".monja-set.toml");
        let config = fs::read(config_path).unwrap_or_default();

        toml::from_slice(&config).map_err(|e| SetConfigError::Deserialization(set_name.clone(), e))
    }

    pub fn save(&self, profile: &MonjaProfile, set_name: &SetName) -> Result<(), SetConfigError> {
        let set_dir = profile.repo_root.join(set_name);
        fs::create_dir_all(&set_dir).map_err(|e| SetConfigError::Save(set_name.clone(), e))?;

        let config_path = set_dir.join(".monja-set.toml");
        let config = toml::to_string(&self)
            .map_err(|e| SetConfigError::Serialization(set_name.clone(), e))?;

        fs::write(config_path, config).map_err(|e| SetConfigError::Save(set_name.clone(), e))
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize)]
pub struct SetName(pub String);
impl Display for SetName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for SetName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> AsRef<T> for SetName
where
    T: ?Sized,
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}

#[derive(Debug)]
pub(crate) struct SetShortcut(RelativePathBuf);
impl SetShortcut {
    pub fn from_path(path: PathBuf) -> Result<Self, SetShortcutError> {
        let rel = RelativePathBuf::from_path(&path)
            .map_err(|e| SetShortcutError::NotRelative(path.clone(), e))?;

        let traversal_detection = rel.to_logical_path(".");
        if traversal_detection.as_path().as_os_str().is_empty() && !path.as_os_str().is_empty() {
            return Err(SetShortcutError::TraversalToParent(path));
        }

        Ok(SetShortcut(rel))
    }
}

// TODO: do a pass on all asrefs and consider deref as well
impl<T> AsRef<T> for SetShortcut
where
    T: ?Sized,
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}

impl Deref for SetShortcut {
    type Target = RelativePath;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Error, Debug)]
pub enum SetConfigError {
    #[error("Unable to deserialize .monja-set.toml for set '{0}'.")]
    Deserialization(SetName, #[source] toml::de::Error),
    #[error("Unable to serialize .monja-set.toml for set '{0}'.")]
    Serialization(SetName, #[source] toml::ser::Error),
    #[error("Unable to save .monja-set.toml for set '{0}'.")]
    Save(SetName, #[source] std::io::Error),
}

#[derive(Error, Debug)]
pub enum SetShortcutError {
    #[error("Shortcut does not appear to be a relative path: {0}")]
    NotRelative(PathBuf, #[source] relative_path::FromPathError),
    #[error("Shortcut appears to be trying to traverse above the profile directory: {0}")]
    TraversalToParent(PathBuf),
}

#[derive(Error, Debug)]
pub enum StateInitializationError {
    #[error("Unable to read the state of the repo.")]
    ReadSetDirs(#[source] std::io::Error),
    #[error("Unable to convert dir name into set name: {0:?}")]
    NonUtf8Path(std::ffi::OsString),
    #[error("Set shortcut is invalid.")]
    SetShortcutInvalid(#[from] SetShortcutError),
    #[error("Error in walking directory for set '{0}'.")]
    DirectoryWalk(SetName, #[source] walkdir::Error),
    #[error("Unable to load set config.")]
    SetConfig(#[from] SetConfigError),
    #[error("Unable to parse set's shortcut: {0}")]
    InvalidShortcut(PathBuf, #[source] relative_path::FromPathError),
}

pub(crate) fn initialize_full_state(
    profile: &MonjaProfile,
) -> Result<RepoState, Vec<StateInitializationError>> {
    // while we'll prefer to collect errors into a vector, there's no point in continuing if we can't read this dir.
    let read_dir = fs::read_dir(&profile.repo_root)
        .map_err(|e| vec![StateInitializationError::ReadSetDirs(e)])?;

    let mut set_info = Vec::new();
    let mut errors = Vec::new();

    for result in read_dir {
        match result {
            Err(err) => errors.push(StateInitializationError::ReadSetDirs(err)),
            Ok(e) if e.path().is_dir() => {
                match e.file_name().into_string() {
                    Ok(str) => set_info.push((SetName(str), e.path())),
                    Err(initial) => errors.push(StateInitializationError::NonUtf8Path(initial)),
                };
            }
            _ => (), // non-dirs
        };
    }

    let mut sets = HashMap::with_capacity(set_info.len());
    for (set_name, set_path) in set_info {
        let set = load_set_state(profile, &set_name, set_path);
        match set {
            Ok(set) => _ = sets.insert(set_name, set),
            Err(err) => errors.push(err),
        };
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(RepoState { sets })
}

fn load_set_state(
    profile: &MonjaProfile,
    set_name: &SetName,
    set_path: PathBuf,
) -> Result<Set, StateInitializationError> {
    let set_config = SetConfig::load(profile, set_name)?;

    let shortcut = set_config.shortcut.unwrap_or("".into());
    let shortcut = SetShortcut::from_path(shortcut)?;

    let root = AbsolutePath::for_existing_path(&profile.repo_root.join(set_name))
        .expect("This function gets called after reading dirs in repo root.");

    let mut locally_mapped_files = HashMap::new();
    for entry in WalkDir::new(&set_path) {
        let entry =
            entry.map_err(|e| StateInitializationError::DirectoryWalk(set_name.clone(), e))?;
        if entry.file_type().is_file() && !crate::is_monja_special_file(entry.path()) {
            let path_in_set = entry.path().strip_prefix(&set_path).expect(
                "The entry path should start with set_path, since that's what we called it with.",
            );
            let path_in_set = RelativePathBuf::from_path(path_in_set)
                .expect("Stripping of the prefix should make path relative");
            let path = FilePath::new(&shortcut, path_in_set);

            let file = File {
                owning_set: set_name.clone(),
                path,
            };

            locally_mapped_files.insert(file.path.local_path.clone(), file);
        }
        // ignore dirs
    }

    Ok(Set {
        _name: set_name.clone(),
        shortcut,
        root,
        locally_mapped_files,
    })
}
