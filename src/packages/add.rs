use std::collections::HashSet;

use thiserror::Error;

use crate::{ExecutionOptions, MonjaProfile, set};

use super::repo;

#[derive(Error, Debug)]
pub enum AddError {
    #[error("Unable to initialize repo state.")]
    RepoStateInitialization(Vec<repo::StateInitializationError>),

    #[error("Set not found in repo.")]
    SetNotFound(set::SetName),

    #[error("Unable to load or save .monja-set.toml.")]
    SetConfig(#[from] set::SetConfigError),
}

#[derive(Debug)]
pub struct AddSuccess {
    pub set_name: set::SetName,
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
    repo.sets
        .get(&set_name)
        .ok_or_else(|| AddError::SetNotFound(set_name.clone()))?;

    let mut config = set::SetConfig::load(profile, &set_name)?;
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
        config.save(profile, &set_name)?;
    }

    Ok(AddSuccess {
        set_name,
        added,
        already_present,
    })
}
