use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{MonjaProfile, local, repo};

#[derive(Serialize, Deserialize)]
pub(crate) struct FileIndex {
    #[serde(flatten)]
    set_mapping: HashMap<local::FilePath, repo::SetName>,
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

        let index = fs::read(index_path).map_err(|e| FileIndexError::Read(kind.clone(), e))?;

        toml::from_slice(&index).map_err(|e| FileIndexError::Deserialization(kind, e))
    }

    pub(crate) fn new() -> FileIndex {
        FileIndex {
            set_mapping: HashMap::new(),
        }
    }

    pub(crate) fn save(
        &self,
        profile: &MonjaProfile,
        kind: IndexKind,
    ) -> Result<(), FileIndexError> {
        let path = FileIndex::path(profile, &kind);

        fs::write(
            &path,
            toml::to_string(self).map_err(|e| FileIndexError::Serialization(kind.clone(), e))?,
        )
        .map_err(|e| FileIndexError::Write(IndexKind::Current, e))
    }

    pub(crate) fn tracks(&self, local_file: &local::FilePath) -> bool {
        self.set_mapping.contains_key(local_file)
    }

    pub(crate) fn take(&mut self, local_file: &local::FilePath) -> Option<repo::SetName> {
        self.set_mapping.remove(local_file)
    }

    pub(crate) fn set(&mut self, local_file: local::FilePath, owning_set: repo::SetName) {
        self.set_mapping.insert(local_file, owning_set);
    }

    pub(crate) fn into_files_not_in(
        self,
        profile: &MonjaProfile,
        other: &FileIndex,
    ) -> Result<Vec<local::FilePath>, local::LocalWalkError> {
        let unignored_files: Result<HashSet<local::FilePath>, local::LocalWalkError> =
            local::walk(profile).collect();
        let unignored_files: HashSet<local::FilePath> = unignored_files?;

        let mut old_files_since_last_pull: Vec<local::FilePath> = self
            .set_mapping
            .into_keys()
            .filter(|f| unignored_files.contains(f))
            .filter(|f| !other.tracks(f))
            .collect();
        old_files_since_last_pull.sort_by(|l, r| l.as_ref().cmp(r.as_ref()));
        Ok(old_files_since_last_pull)
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
) -> Result<Vec<local::FilePath>, FileIndexError> {
    let curr_index = FileIndex::load(profile, IndexKind::Current)?;
    let prev_index = FileIndex::load(profile, IndexKind::Previous)?;

    let old_files = prev_index.into_files_not_in(profile, &curr_index)?;
    Ok(old_files)
}
#[derive(Error, Debug)]
#[error("Error when walking local files.")]
// this will also be a rare case of using anyhow in this crate (we use it plenty in main).
// we want to hide the ignore crate's details.
pub struct LocalWalkError(#[from] pub(super) anyhow::Error);

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

    #[error("Error when walking local files to find out which are ignored.")]
    LocalWalk(#[from] LocalWalkError),
}
