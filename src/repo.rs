use std::{collections::HashMap, fmt::Display, fs, path::PathBuf};

use relative_path::{RelativePath, RelativePathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use walkdir::WalkDir;

use crate::{AbsolutePath, MonjaProfile, local};

// TODO: might as well deref to str and add a from
#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize)]
pub struct SetName(pub String);
impl Display for SetName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub(crate) struct RepoState {
    pub sets: HashMap<SetName, Set>,
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
        let config_path = profile
            .repo_root
            .as_ref()
            .join(&set_name.0)
            .join(".monja-set.toml");
        let config = fs::read(config_path).unwrap_or_default();

        toml::from_slice(&config).map_err(|e| SetConfigError::Deserialization(set_name.clone(), e))
    }

    pub fn save(
        &self,
        profile: &crate::MonjaProfile,
        set_name: &SetName,
    ) -> Result<(), SetConfigError> {
        let set_dir = profile.repo_root.as_ref().join(&set_name.0);
        fs::create_dir_all(&set_dir).map_err(|e| SetConfigError::Save(set_name.clone(), e))?;

        let config_path = set_dir.join(".monja-set.toml");
        let config = toml::to_string(&self)
            .map_err(|e| SetConfigError::Serialization(set_name.clone(), e))?;

        fs::write(config_path, config).map_err(|e| SetConfigError::Save(set_name.clone(), e))
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

pub(crate) struct Set {
    pub _name: SetName,
    // TODO: validate and test that this is a child of the local root, to avoid directory traversal attacks
    pub shortcut: RelativePathBuf,
    pub root: AbsolutePath,
    // directories: HashMap<ObjectPath, Directory>,
    pub locally_mapped_files: HashMap<local::FilePath, File>,
}
impl Set {
    pub(crate) fn tracks_file(&self, local_file: &local::FilePath) -> bool {
        self.locally_mapped_files.contains_key(local_file)
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

// TODO: am expecting this design to become necessary, though it isnt right now
// for instance, we might track perms or cleanup state through these
// additionally, the directory struct might become the representation of a future monja-dir.toml
pub(crate) struct File {
    pub owning_set: SetName,
    pub path: FilePath,
}
impl File {}

pub(crate) struct _Directory {}
impl _Directory {}

pub(crate) fn initialize_full_state(
    profile: &MonjaProfile,
) -> Result<RepoState, Vec<StateInitializationError>> {
    // while we'll prefer to collect errors into a vector, there's no point in continuing if we can't read this dir.
    let read_dir = fs::read_dir(profile.repo_root.as_ref())
        .map_err(|e| vec![StateInitializationError::Io(e)])?;

    let mut set_info = Vec::new();
    let mut errors = Vec::new();

    // versus ::partition, less vector allocations, since we would need a second pass+vector on read_dir Ok()s
    for result in read_dir {
        match result {
            Err(err) => errors.push(err.into()),
            Ok(res) if res.path().is_dir() => {
                match res.file_name().into_string() {
                    Ok(str) => set_info.push((SetName(str), res.path())),
                    Err(initial) => errors.push(StateInitializationError::NonUtf8Path(initial)),
                };
            }
            _ => {} // dirs in particular
        };
    }
    let set_info = set_info;

    let mut sets = HashMap::with_capacity(set_info.len());
    for (set_name, set_path) in set_info {
        // using a lambda so we can use ?, versus branches to populate 2 containers in multiple places in this fragment
        let set = (|| {
            let set_config = SetConfig::load(profile, &set_name)?;

            let shortcut = set_config.shortcut.unwrap_or("".into());
            let shortcut = RelativePathBuf::from_path(&shortcut)
                .map_err(|e| StateInitializationError::InvalidShortcut(shortcut, e))?;

            // TODO: get rid of these .0s. don't want  asref deref tho
            let root =
                AbsolutePath::for_existing_path(&profile.repo_root.as_ref().join(&set_name.0))
                    .expect("These are from the above loop on folders in the repo root.");

            let mut locally_mapped_files = HashMap::new();
            for entry in WalkDir::new(&set_path) {
                let entry = entry
                    .map_err(|e| StateInitializationError::DirectoryWalk(set_name.clone(), e))?;
                if entry.file_type().is_file() {
                    let path_in_set = entry
                        .path()
                        .strip_prefix(&set_path)
                        .expect("The entry path should start with set_path, since that's what we called it with.");
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
        })();
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

#[derive(Error, Debug)]
pub enum StateInitializationError {
    #[error("Unable to read the state of the repo.")]
    Io(#[from] std::io::Error),
    #[error("Unable to convert dir name into set name: {0:?}")]
    NonUtf8Path(std::ffi::OsString),
    #[error("Error in walking directory for set '{0}'.")]
    DirectoryWalk(SetName, #[source] walkdir::Error),
    #[error("Unable to load set config.")]
    SetConfig(#[from] SetConfigError),
    #[error("Unable to parse set's shortcut: {0}")]
    InvalidShortcut(PathBuf, #[source] relative_path::FromPathError),
}
