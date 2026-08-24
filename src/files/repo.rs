use std::{collections::HashMap, path::PathBuf};

use relative_path::RelativePathBuf;
use thiserror::Error;
use walkdir::WalkDir;

use crate::{AbsolutePath, MonjaProfile, RepoName, set};

use super::local;

pub(crate) type RepoState = set::SetStates<Set>;

impl RepoState {
    pub(crate) fn get_owning_set<'a>(
        &self,
        profile: &'a MonjaProfile,
        file: &local::FilePath,
    ) -> Option<&'a set::SetName> {
        profile
            .config
            .target_sets
            .iter()
            .rev()
            .find(|name| self.sets.get(*name).is_some_and(|s| s.tracks_file(file)))
    }

    pub(crate) fn get_set(&self, name: &set::SetName) -> Result<&Set, set::SetLookupError> {
        self.get(name)
    }
}

pub(crate) struct Set {
    pub name: set::SetName,
    pub repo: RepoName,
    pub shortcut: set::SetShortcut,
    pub root: AbsolutePath,
    // directories: HashMap<ObjectPath, Directory>,
    pub locally_mapped_files: HashMap<local::FilePath, File>,
}

impl Set {
    pub(crate) fn tracks_file(&self, local_path: &local::FilePath) -> bool {
        self.locally_mapped_files.contains_key(local_path)
    }

    // returns PathBuf because AbsolutePath requires the file exist
    pub(crate) fn get_repo_absolute_path_for(
        &self,
        local_path: &local::FilePath,
    ) -> Result<PathBuf, SetPathError> {
        Ok(self
            .get_repo_relative_path_for(local_path)?
            .to_path(&self.root))
    }

    pub(crate) fn get_repo_relative_path_for(
        &self,
        local_path: &local::FilePath,
    ) -> Result<RelativePathBuf, SetPathError> {
        let path = self.shortcut.relative(local_path);

        //  for `shortcut=foo/bar; path=foo/baz.file` we should fail
        match path.components().next() {
            Some(relative_path::Component::ParentDir) => Err(SetPathError::OutsideOfSet {
                shortcut: self.shortcut.to_path(""),
                path: local_path.clone().into(),
            }),
            None => Err(SetPathError::NotSure {
                shortcut: self.shortcut.to_path(""),
                path: local_path.clone().into(),
            }),
            _ => Ok(path),
        }
    }
}

#[derive(Error, Debug)]
pub enum SetPathError {
    #[error("The local file path of '{path}' fall outside of the set's shortcut: {shortcut}")]
    OutsideOfSet { shortcut: PathBuf, path: PathBuf },

    #[error("Have an empty relative path for some reason. Shortcut: {shortcut}; Path: {path}")]
    NotSure { shortcut: PathBuf, path: PathBuf },
}

pub(crate) struct FilePath {
    pub path_in_set: RelativePathBuf,
    pub local_path: local::FilePath,
}

impl FilePath {
    fn new(shortcut: &set::SetShortcut, path_in_set: RelativePathBuf) -> FilePath {
        let local_path = local::FilePath::for_set(shortcut, &path_in_set);

        FilePath {
            path_in_set,
            local_path,
        }
    }
}

pub(crate) struct File {
    pub owning_set: set::SetName,
    pub path: FilePath,
}

#[derive(Error, Debug)]
pub enum StateInitializationError {
    #[error("Unable to read the state of repo '{0}'.")]
    ReadSetDirs(RepoName, #[source] std::io::Error),
    #[error("Unable to convert dir name into set name: {0:?}")]
    NonUtf8Path(std::ffi::OsString),
    #[error("Set shortcut is invalid.")]
    SetShortcutInvalid(#[from] set::SetShortcutError),
    #[error("Error in walking directory for set '{0}'.")]
    DirectoryWalk(set::SetName, #[source] walkdir::Error),
    #[error("Unable to load set config.")]
    SetConfig(#[from] set::SetConfigError),
    #[error("A targeted set could not be resolved to a single repo.")]
    AmbiguousSet(#[source] set::SetLookupError),
}

// hand-written rather than `#[from]` since it flattens `DiscoverSetsError`'s variants into this
// enum's own matching variants, instead of wrapping them -- tests (and callers generally) match
// on the flat shape, e.g. `RepoStateInitializationError::ReadSetDirs(..)`.
impl From<set::DiscoverSetsError> for StateInitializationError {
    fn from(err: set::DiscoverSetsError) -> Self {
        match err {
            set::DiscoverSetsError::ReadSetDirs(repo, e) => {
                StateInitializationError::ReadSetDirs(repo, e)
            }
            set::DiscoverSetsError::NonUtf8Path(e) => StateInitializationError::NonUtf8Path(e),
        }
    }
}

pub(crate) fn initialize_full_state(
    profile: &MonjaProfile,
) -> Result<RepoState, Vec<StateInitializationError>> {
    set::load_sets(
        profile,
        StateInitializationError::AmbiguousSet,
        load_set_state,
    )
}

fn load_set_state(
    set_name: &set::SetName,
    location: &set::SetLocation,
) -> Result<Set, StateInitializationError> {
    let set_config = set::SetConfig::load(&location.root, set_name)?;

    let shortcut = set_config.shortcut.unwrap_or("".into());
    let shortcut = set::SetShortcut::from_path(shortcut)?;

    let root = AbsolutePath::for_existing_path(&location.root)
        .expect("This function gets called after reading dirs in repo roots.");

    let mut locally_mapped_files = HashMap::new();
    for entry in WalkDir::new(&location.root) {
        let entry =
            entry.map_err(|e| StateInitializationError::DirectoryWalk(set_name.clone(), e))?;
        if entry.file_type().is_file() && !crate::is_monja_special_file(entry.path()) {
            let path_in_set = entry.path().strip_prefix(&location.root).expect(
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
        name: set_name.clone(),
        repo: location.repo.clone(),
        shortcut,
        root,
        locally_mapped_files,
    })
}
