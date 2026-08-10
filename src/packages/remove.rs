use std::collections::HashSet;

use thiserror::Error;

use crate::{ExecutionOptions, MonjaProfile, set};

use super::repo;

#[derive(Error, Debug)]
pub enum RemoveError {
    #[error("Unable to initialize repo state.")]
    RepoStateInitialization(Vec<repo::StateInitializationError>),

    #[error("Set not found in repo.")]
    SetNotFound(set::SetName),

    #[error("Unable to load or save .monja-set.toml.")]
    SetConfig(#[from] set::SetConfigError),
}

#[derive(Debug)]
pub struct RemoveSuccess {
    pub set_name: set::SetName,
    pub removed: Vec<String>,
    pub not_present: Vec<String>,
}

pub fn remove(
    profile: &MonjaProfile,
    opts: &ExecutionOptions,
    set_name: set::SetName,
    packages: Vec<String>,
) -> Result<RemoveSuccess, RemoveError> {
    let repo = repo::initialize_state(profile).map_err(RemoveError::RepoStateInitialization)?;
    repo.sets
        .get(&set_name)
        .ok_or_else(|| RemoveError::SetNotFound(set_name.clone()))?;

    let mut config = set::SetConfig::load(profile, &set_name)?;
    let existing: HashSet<&str> = config.packages.iter().map(String::as_str).collect();

    let mut removed = Vec::new();
    let mut not_present = Vec::new();
    for package in packages {
        if existing.contains(package.as_str()) {
            removed.push(package);
        } else {
            not_present.push(package);
        }
    }

    if !opts.dry_run && !removed.is_empty() {
        config.packages.retain(|p| !removed.contains(p));
        config.save(profile, &set_name)?;
    }

    Ok(RemoveSuccess {
        set_name,
        removed,
        not_present,
    })
}
