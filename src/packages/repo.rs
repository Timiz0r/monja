use std::path::PathBuf;

use thiserror::Error;

use crate::{MonjaProfile, RepoName, set};

pub(crate) type RepoState = set::SetStates<Set>;

impl RepoState {
    pub(crate) fn get_set(&self, name: &set::SetName) -> Result<&Set, set::SetLookupError> {
        self.get(name)
    }
}

pub(crate) struct Set {
    pub repo: RepoName,
    // unlike files' Set, this isn't used to locate content -- it's what lets `add`/`remove`
    // write back to the right `.monja-set.toml` now that a set name alone doesn't locate one.
    pub root: PathBuf,
    pub packages: Vec<String>,
}

#[derive(Error, Debug)]
pub enum StateInitializationError {
    #[error("Unable to read the state of repo '{0}'.")]
    ReadSetDirs(RepoName, #[source] std::io::Error),
    #[error("Unable to convert dir name into set name: {0:?}")]
    NonUtf8Path(std::ffi::OsString),
    #[error("Unable to load set config.")]
    SetConfig(#[from] set::SetConfigError),
    #[error("A targeted set could not be resolved to a single repo.")]
    AmbiguousSet(#[source] set::SetLookupError),
}

// hand-written rather than `#[from]` -- see the identical note on files::repo's version.
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

// unlike file's version, this never walks a set's files -- packages are just names declared
// directly in `.monja-set.toml`, so loading a set's config is all that's needed.
pub(crate) fn initialize_state(
    profile: &MonjaProfile,
) -> Result<RepoState, Vec<StateInitializationError>> {
    set::load_sets(
        profile,
        StateInitializationError::AmbiguousSet,
        |set_name, location| {
            let config = set::SetConfig::load(&location.root, set_name)?;

            Ok(Set {
                repo: location.repo.clone(),
                root: location.root.clone(),
                packages: config.packages,
            })
        },
    )
}
