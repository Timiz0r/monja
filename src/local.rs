use std::{
    collections::HashMap,
    fmt::Display,
    fs,
    path::{Path, PathBuf},
};

use crate::{MonjaProfile, SetName, repo};

use ignore::WalkBuilder;
use relative_path::{RelativePath, RelativePathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub(crate) struct LocalState {
    pub files_to_push: HashMap<repo::SetName, Vec<FilePath>>,
    pub files_with_missing_sets: HashMap<repo::SetName, Vec<FilePath>>,
    pub missing_files: HashMap<repo::SetName, Vec<FilePath>>,
    pub untracked_files: Vec<FilePath>,
    // note that these same files may be in untracked_files.
    pub old_files_since_last_pull: Vec<FilePath>,
}

pub(crate) fn retrieve_state(
    profile: &MonjaProfile,
    repo: &repo::RepoState,
) -> Result<LocalState, StateInitializationError> {
    let mut curr_index = FileIndex::load(profile, IndexKind::Current)?;

    let mut files_to_push = HashMap::with_capacity(repo.sets.len());
    let mut untracked_files = Vec::new();
    let mut files_with_missing_sets = HashMap::with_capacity(repo.sets.len());
    let mut missing_files = HashMap::with_capacity(repo.sets.len());

    let prev_index = FileIndex::load(profile, IndexKind::Previous)?;
    let old_files_since_last_pull = index_diff(&curr_index, prev_index);

    for local_path in walk(profile) {
        let local_path = local_path?;
        let Some(set_name) = curr_index.take(&local_path) else {
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
        files_with_missing_sets,
        missing_files,
        untracked_files,
        old_files_since_last_pull,
    })
}

#[derive(Serialize, Deserialize)]
pub(crate) struct FileIndex {
    #[serde(flatten)]
    set_mapping: HashMap<FilePath, repo::SetName>,
}

impl FileIndex {
    pub(crate) fn load(
        profile: &MonjaProfile,
        kind: IndexKind,
    ) -> Result<FileIndex, FileIndexError> {
        let index_path = FileIndex::path(profile, &kind);

        if !index_path.exists() {
            return Ok(FileIndex {
                set_mapping: HashMap::new(),
            });
        }

        let index = std::fs::read(index_path).map_err(|e| FileIndexError::Read(kind.clone(), e))?;

        toml::from_slice(&index).map_err(|e| FileIndexError::Deserialization(kind, e))
    }

    pub(crate) fn new() -> FileIndex {
        FileIndex {
            set_mapping: HashMap::new(),
        }
    }

    pub(crate) fn update(&self, profile: &MonjaProfile) -> Result<(), FileIndexError> {
        let curr_path = FileIndex::path(profile, &IndexKind::Current);

        if curr_path.exists() {
            fs::copy(&curr_path, FileIndex::path(profile, &IndexKind::Previous))
                .map_err(FileIndexError::CopyToPrevious)?;
        }

        std::fs::write(
            &curr_path,
            toml::to_string(self)
                .map_err(|e| FileIndexError::Serialization(IndexKind::Current, e))?,
        )
        .map_err(|e| FileIndexError::Write(IndexKind::Current, e))
    }

    pub(crate) fn save(
        &self,
        profile: &MonjaProfile,
        kind: &IndexKind,
    ) -> Result<(), FileIndexError> {
        let path = FileIndex::path(profile, kind);

        std::fs::write(
            &path,
            toml::to_string(self).map_err(|e| FileIndexError::Serialization(kind.clone(), e))?,
        )
        .map_err(|e| FileIndexError::Write(IndexKind::Current, e))
    }

    pub(crate) fn tracks(&self, local_file: &FilePath) -> bool {
        self.set_mapping.contains_key(local_file)
    }

    pub(crate) fn take(&mut self, local_file: &FilePath) -> Option<repo::SetName> {
        self.set_mapping.remove(local_file)
    }

    pub(crate) fn set(&mut self, local_file: FilePath, owning_set: SetName) {
        self.set_mapping.insert(local_file, owning_set);
    }

    // not an AbsolutePath because the index may not exist
    fn path(profile: &MonjaProfile, kind: &IndexKind) -> PathBuf {
        profile.data_root.join(kind.file_name())
    }
}

#[derive(Debug, Clone)]
pub enum IndexKind {
    Current,
    Previous,
}

impl IndexKind {
    pub(crate) fn file_name(&self) -> &Path {
        match self {
            IndexKind::Current => "monja-index.toml".as_ref(),
            IndexKind::Previous => "monja-index-prev.toml".as_ref(),
        }
    }
}

impl Display for IndexKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.file_name().display())
    }
}

// while we could get rid of this in favor of using LocalState,
// it's a lot cheaper to do it this way, since we only need indices instead of both local and repo state.
pub(crate) fn old_files_since_last_pull(
    profile: &MonjaProfile,
) -> Result<Vec<FilePath>, FileIndexError> {
    let curr_index = FileIndex::load(profile, IndexKind::Current)?;
    let prev_index = FileIndex::load(profile, IndexKind::Previous)?;

    let old_files = index_diff(&curr_index, prev_index);
    Ok(old_files)
}

#[derive(Hash, PartialEq, Eq, Clone, Serialize, Deserialize, Debug)]
#[serde(try_from = "std::path::PathBuf")]
#[serde(into = "std::path::PathBuf")]
pub(crate) struct FilePath(RelativePathBuf);

impl FilePath {
    pub(crate) fn new(object_path: RelativePathBuf) -> FilePath {
        FilePath(object_path)
    }

    pub(crate) fn to_path(&self, base: &Path) -> PathBuf {
        self.0.to_path(base)
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
    #[error("Unable to read monja-index.toml.")]
    FileIndex(#[from] FileIndexError),

    // an alternative is to aggregate these and return them as part of the result
    // instead, am opting for making extra sure we have an accurate picture of local state by failing fast
    #[error("Error when walking local files.")]
    LocalWalk(#[from] LocalWalkError),
}

#[derive(Error, Debug)]
pub enum FileIndexError {
    #[error("Unable to read the file index.")]
    Read(IndexKind, #[source] std::io::Error),
    #[error("Unable to write the file index.")]
    Write(IndexKind, #[source] std::io::Error),
    #[error("Unable to copy the current file index to the previous file index.")]
    CopyToPrevious(#[source] std::io::Error),
    #[error("Unable to deserialize monja-index.toml.")]
    Deserialization(IndexKind, #[source] toml::de::Error),
    #[error("Unable to serialize monja-index.toml.")]
    Serialization(IndexKind, #[source] toml::ser::Error),
}

#[derive(Error, Debug)]
#[error("Error when walking local files.")]
// this will also be a rare case of using anyhow in this crate (we use it plenty in main).
// we want to hide the ignore crate's details.
pub struct LocalWalkError(#[from] anyhow::Error);

fn walk(profile: &MonjaProfile) -> impl Iterator<Item = Result<FilePath, LocalWalkError>> {
    let local_root = &profile.local_root;
    let repo_root = &profile.repo_root;
    let walker = WalkBuilder::new(local_root)
        .standard_filters(false)
        .add_custom_ignore_filename(".monjaignore")
        .follow_links(false)
        .hidden(false)
        .build();
    walker
        // not returning a Result<Iter, ...> because we we're opting to fail fast on the first walk error.
        // using map_or in this way is the only way I can think of at the moment
        .filter(|r| r.as_ref().map_or(true, |e| e.path().is_file()))
        .filter(move |r| {
            r.as_ref()
                .map_or(true, |e| !e.path().starts_with(repo_root))
        })
        .filter(|r| {
            r.as_ref()
                .map_or(true, |e| !crate::is_monja_special_file(e.path()))
        })
        .map(move |entry| {
            // would be convenient to map path out earlier, but that requires a clone
            // because the path comes from a dropped Entry.
            let entry = entry.map_err(|e| LocalWalkError(e.into()))?;
            let path = entry
                .path()
                .strip_prefix(local_root)
                .expect("Should naturally be a prefix.");
            Ok(FilePath(
                RelativePathBuf::from_path(path).expect("Generated a relative path."),
            ))
        })
}

fn index_diff(curr_index: &FileIndex, prev_index: FileIndex) -> Vec<FilePath> {
    let mut old_files_since_last_pull: Vec<FilePath> = prev_index
        .set_mapping
        .into_keys()
        .filter(|f| !curr_index.tracks(f))
        .collect();
    old_files_since_last_pull.sort_by(|l, r| l.as_ref().cmp(r.as_ref()));
    old_files_since_last_pull
}
