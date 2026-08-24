use std::collections::HashSet;

use thiserror::Error;

use crate::{ExecutionOptions, MonjaProfile, set};

use super::repo;

#[derive(Error, Debug)]
pub enum RemoveError {
    #[error("Unable to initialize repo state:{}", crate::format_errors(.0))]
    RepoStateInitialization(Vec<repo::StateInitializationError>),

    #[error("Set not found in repo.")]
    SetNotFound(set::SetName),

    #[error("Set exists in multiple repos.")]
    AmbiguousSet(#[source] set::SetLookupError),

    #[error("Unable to load or save .monja-set.toml.")]
    SetConfig(#[from] set::SetConfigError),
}

#[derive(Debug)]
pub struct RemoveSuccess {
    pub set_name: set::SetName,
    pub repo: crate::RepoName,
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
    let set = repo.get_set(&set_name).map_err(|e| match e {
        set::SetLookupError::NotFound(name) => RemoveError::SetNotFound(name),
        e => RemoveError::AmbiguousSet(e),
    })?;

    let mut config = set::SetConfig::load(&set.root, &set_name)?;
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
        config.save(&set.root, &set_name)?;
    }

    Ok(RemoveSuccess {
        set_name,
        repo: set.repo.clone(),
        removed,
        not_present,
    })
}
