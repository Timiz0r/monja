use std::{collections::HashMap, io, path::PathBuf};

use crate::{AbsolutePath, MonjaProfile, repo};

use ignore::WalkBuilder;
use relative_path::{RelativePath, RelativePathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize, Debug)]
#[serde(from = "std::path::PathBuf")]
#[serde(into = "std::path::PathBuf")]
pub(crate) struct FilePath(RelativePathBuf);
impl FilePath {
    pub(crate) fn new(object_path: RelativePathBuf) -> FilePath {
        FilePath(object_path)
    }

    pub(crate) fn into_relative_path_buf(self) -> RelativePathBuf {
        self.0
    }

    pub(crate) fn _as_path_buf(&self) -> PathBuf {
        self.0.to_path("")
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
impl From<std::path::PathBuf> for FilePath {
    fn from(value: std::path::PathBuf) -> Self {
        FilePath(RelativePathBuf::from_path(value).expect("Path is a path"))
    }
}

pub(crate) struct LocalState {
    pub files_to_push: Vec<(repo::SetName, Vec<FilePath>)>,
    pub _untracked_files: Vec<FilePath>,
    pub missing_sets: Vec<(repo::SetName, Vec<FilePath>)>,
    pub missing_files: Vec<(repo::SetName, Vec<FilePath>)>,
}

pub(crate) fn retrieve_state(
    profile: &MonjaProfile,
    repo: &repo::RepoState,
) -> Result<LocalState, StateInitializationError> {
    let index = FileIndex::load(&profile.local_root)?;

    let mut files_to_push = HashMap::with_capacity(repo.sets.len());
    let mut untracked_files = Vec::new();
    // so signifies the files indicating the set should exist
    let mut missing_sets = HashMap::with_capacity(repo.sets.len());
    let mut missing_files = HashMap::with_capacity(repo.sets.len());

    for local_path in walk(&profile.local_root) {
        let local_path = local_path?;
        let Some(set_name) = index.get(&local_path) else {
            untracked_files.push(local_path);
            continue;
        };
        // note that single clone works thanks to early exit continues
        let set_name = set_name.clone();
        let Some(set) = repo.sets.get(&set_name) else {
            missing_sets
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
        files_to_push: files_to_push.into_iter().collect(),
        _untracked_files: untracked_files,
        missing_sets: missing_sets.into_iter().collect(),
        missing_files: missing_files.into_iter().collect(),
    })
}

fn walk(root: &AbsolutePath) -> impl Iterator<Item = io::Result<FilePath>> {
    let walker = WalkBuilder::new(root.as_ref())
        .standard_filters(false)
        .add_custom_ignore_filename(".monjaignore")
        .follow_links(true)
        .build();
    walker.flatten().map(|entry| {
        Ok(FilePath(
            RelativePathBuf::from_path(entry.path())
                .expect("The walker shouldn't produce absolute paths."),
        ))
    })
}

#[derive(Serialize, Deserialize)]
pub(crate) struct FileIndex {
    #[serde(flatten)]
    set_mapping: HashMap<FilePath, repo::SetName>,
}
impl FileIndex {
    fn load(root: &AbsolutePath) -> Result<FileIndex, FileIndexError> {
        let index = std::fs::read(root.as_ref().join(".monja-index.toml"))?;

        toml::from_slice(&index).map_err(|e| e.into())
    }

    // preferring a vec for ownership and len()
    pub(crate) fn new(set_mapping: HashMap<FilePath, repo::SetName>) -> FileIndex {
        FileIndex { set_mapping }
    }

    pub(crate) fn save(&self, root: &AbsolutePath) -> Result<(), FileIndexError> {
        std::fs::write(
            root.as_ref().join(".monja-index.toml"),
            toml::to_string(self)?,
        )
        .map_err(|e| e.into())
    }

    fn get(&self, path: &FilePath) -> Option<&repo::SetName> {
        self.set_mapping.get(path)
    }
}

#[derive(Error, Debug)]
pub enum StateInitializationError {
    #[error("Unable to read the state of the local directory.")]
    Io(#[from] std::io::Error),
    #[error("Unable to read .monja-index.toml.")]
    FileIndex(#[from] FileIndexError),
}

#[derive(Error, Debug)]
pub enum FileIndexError {
    #[error("Unable to read the state of .monja-index.toml.")]
    Io(#[from] std::io::Error),
    #[error("Unable to deserialize .monja-index.toml.")]
    Deserialization(#[from] toml::de::Error),
    #[error("Unable to serialize .monja-index.toml.")]
    Serialization(#[from] toml::ser::Error),
}
