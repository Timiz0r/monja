use std::collections::HashMap;

use thiserror::Error;

use crate::{MonjaProfile, set};

pub(crate) struct RepoState {
    pub sets: HashMap<set::SetName, Set>,
}

pub(crate) struct Set {
    pub packages: Vec<String>,
}

#[derive(Error, Debug)]
pub enum StateInitializationError {
    #[error("Unable to read the state of the repo.")]
    ReadSetDirs(#[source] std::io::Error),
    #[error("Unable to convert dir name into set name: {0:?}")]
    NonUtf8Path(std::ffi::OsString),
    #[error("Unable to load set config.")]
    SetConfig(#[from] set::SetConfigError),
}

// hand-written rather than `#[from]` -- see the identical note on files::repo's version.
impl From<set::DiscoverSetsError> for StateInitializationError {
    fn from(err: set::DiscoverSetsError) -> Self {
        match err {
            set::DiscoverSetsError::ReadSetDirs(e) => StateInitializationError::ReadSetDirs(e),
            set::DiscoverSetsError::NonUtf8Path(e) => StateInitializationError::NonUtf8Path(e),
        }
    }
}

// unlike file's version, this never walks a set's files -- packages are just names declared
// directly in `.monja-set.toml`, so loading a set's config is all that's needed.
pub(crate) fn initialize_state(
    profile: &MonjaProfile,
) -> Result<RepoState, Vec<StateInitializationError>> {
    let set_info = set::discover_sets(profile)
        .map_err(|errs| errs.into_iter().map(Into::into).collect::<Vec<_>>())?;

    let mut sets = HashMap::with_capacity(set_info.len());
    let mut errors = Vec::new();
    for (set_name, _set_path) in set_info {
        match set::SetConfig::load(profile, &set_name) {
            Ok(config) => {
                sets.insert(
                    set_name,
                    Set {
                        packages: config.packages,
                    },
                );
            }
            Err(err) => errors.push(StateInitializationError::SetConfig(err)),
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(RepoState { sets })
}
