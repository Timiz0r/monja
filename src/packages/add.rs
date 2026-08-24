use std::collections::HashSet;

use thiserror::Error;

use crate::{ExecutionOptions, MonjaProfile, set};

use super::repo;

#[derive(Error, Debug)]
pub enum AddError {
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
pub struct AddSuccess {
    pub set_name: set::SetName,
    pub repo: crate::RepoName,
    pub added: Vec<String>,
    pub already_present: Vec<String>,
}

pub fn add(
    profile: &MonjaProfile,
    opts: &ExecutionOptions,
    set_name: set::SetName,
    packages: Vec<String>,
) -> Result<AddSuccess, AddError> {
    let repo = repo::initialize_state(profile).map_err(AddError::RepoStateInitialization)?;
    let set = repo.get_set(&set_name).map_err(|e| match e {
        set::SetLookupError::NotFound(name) => AddError::SetNotFound(name),
        e => AddError::AmbiguousSet(e),
    })?;

    let mut config = set::SetConfig::load(&set.root, &set_name)?;
    let existing: HashSet<&str> = config.packages.iter().map(String::as_str).collect();

    let mut added = Vec::new();
    let mut already_present = Vec::new();
    for package in packages {
        if existing.contains(package.as_str()) {
            already_present.push(package);
        } else {
            added.push(package);
        }
    }

    if !opts.dry_run && !added.is_empty() {
        config.packages.extend(added.iter().cloned());
        config.packages.sort();
        config.save(&set.root, &set_name)?;
    }

    Ok(AddSuccess {
        set_name,
        repo: set.repo.clone(),
        added,
        already_present,
    })
}
