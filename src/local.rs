use std::{collections::HashMap, io};

use crate::{MonjaProfile, SetName, repo};

use ignore::WalkBuilder;
use relative_path::{RelativePath, RelativePathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub(crate) struct LocalState {
    pub files_to_push: HashMap<repo::SetName, Vec<FilePath>>,
    pub untracked_files: Vec<FilePath>,
    pub files_with_missing_sets: HashMap<repo::SetName, Vec<FilePath>>,
    pub missing_files: HashMap<repo::SetName, Vec<FilePath>>,
}

pub(crate) fn retrieve_state(
    profile: &MonjaProfile,
    repo: &repo::RepoState,
) -> Result<LocalState, StateInitializationError> {
    let mut index = FileIndex::load(profile)?;

    let mut files_to_push = HashMap::with_capacity(repo.sets.len());
    let mut untracked_files = Vec::new();
    let mut files_with_missing_sets = HashMap::with_capacity(repo.sets.len());
    let mut missing_files = HashMap::with_capacity(repo.sets.len());

    for local_path in walk(profile) {
        let local_path = local_path?;
        let Some(set_name) = index.take(&local_path) else {
            untracked_files.push(local_path);
            continue;
        };

        let Some(set) = repo.sets.get(&set_name) else {
            files_with_missing_sets
                .entry(set_name)
                .or_insert_with(Vec::new)
                .push(local_path);
            continue;
        };

        if !set.tracks_file(&local_path) {
            missing_files
                .entry(set_name)
                .or_insert_with(Vec::new)
                .push(local_path);
            continue;
        }

        files_to_push
            .entry(set_name)
            .or_insert_with(Vec::new)
            .push(local_path);
    }

    Ok(LocalState {
        files_to_push,
        untracked_files,
        files_with_missing_sets,
        missing_files,
    })
}

#[derive(Serialize, Deserialize)]
pub(crate) struct FileIndex {
    #[serde(flatten)]
    set_mapping: HashMap<FilePath, repo::SetName>,
}

impl FileIndex {
    fn load(profile: &MonjaProfile) -> Result<FileIndex, FileIndexError> {
        let index_path = profile.data_root.as_ref().join("monja-index.toml");
        if !index_path.exists() {
            return Ok(FileIndex {
                set_mapping: HashMap::new(),
            });
        }

        let index = std::fs::read(index_path)?;

        toml::from_slice(&index).map_err(|e| e.into())
    }

    pub(crate) fn save(&self, profile: &MonjaProfile) -> Result<(), FileIndexError> {
        std::fs::write(
            profile.data_root.as_ref().join("monja-index.toml"),
            toml::to_string(self)?,
        )
        .map_err(|e| e.into())
    }

    pub(crate) fn new() -> FileIndex {
        FileIndex {
            set_mapping: HashMap::new(),
        }
    }

    pub(crate) fn take(&mut self, local_file: &FilePath) -> Option<repo::SetName> {
        self.set_mapping.remove(local_file)
    }

    pub(crate) fn set(&mut self, local_file: FilePath, owning_set: SetName) {
        self.set_mapping.insert(local_file, owning_set);
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize, Debug)]
#[serde(try_from = "std::path::PathBuf")]
#[serde(into = "std::path::PathBuf")]
pub(crate) struct FilePath(RelativePathBuf);

impl FilePath {
    pub(crate) fn new(object_path: RelativePathBuf) -> FilePath {
        FilePath(object_path)
    }

    pub(crate) fn into_relative_path_buf(self) -> RelativePathBuf {
        self.0
    }
}

impl AsRef<RelativePath> for FilePath {
    fn as_ref(&self) -> &RelativePath {
        &self.0
    }
}

// kinda ideally dont want to do this, but this is easiest way to get it (de)serialized
impl From<FilePath> for std::path::PathBuf {
    fn from(value: FilePath) -> Self {
        value.0.to_path("") // aka dont specify a base and keep it relative
    }
}

impl TryFrom<std::path::PathBuf> for FilePath {
    type Error = relative_path::FromPathError;

    fn try_from(value: std::path::PathBuf) -> Result<Self, Self::Error> {
        Ok(FilePath(RelativePathBuf::from_path(value)?))
    }
}

#[derive(Error, Debug)]
pub enum StateInitializationError {
    #[error("Unable to read the state of the local directory.")]
    Io(#[from] std::io::Error),
    #[error("Unable to read monja-index.toml.")]
    FileIndex(#[from] FileIndexError),
}

#[derive(Error, Debug)]
pub enum FileIndexError {
    #[error("Unable to read the state of monja-index.toml.")]
    Io(#[from] std::io::Error),
    #[error("Unable to deserialize monja-index.toml.")]
    Deserialization(#[from] toml::de::Error),
    #[error("Unable to serialize monja-index.toml.")]
    Serialization(#[from] toml::ser::Error),
}

fn walk(profile: &MonjaProfile) -> impl Iterator<Item = io::Result<FilePath>> {
    let local_root = profile.local_root.as_ref();
    let repo_root = profile.repo_root.as_ref();
    let walker = WalkBuilder::new(local_root)
        .standard_filters(false)
        .add_custom_ignore_filename(".monjaignore")
        .follow_links(false)
        .hidden(false)
        .build();
    walker
        .flatten()
        // note that WalkBuilder's filter will filter out directories from being walked, so we instead filter here
        .filter(|e| e.path().is_file())
        .filter(move |e| !e.path().starts_with(repo_root))
        .filter(|e| !crate::is_monja_special_file(e.path()))
        .map(move |entry| {
            // would be convenient to map path out earlier, but that requires a clone
            // because the path comes from a dropped Entry.
            let path = entry
                .path()
                .strip_prefix(local_root)
                .expect("Should naturally be a prefix.");
            Ok(FilePath(
                RelativePathBuf::from_path(path).expect("Generated a relative path."),
            ))
        })
}
