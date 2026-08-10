use thiserror::Error;

use crate::{MonjaProfile, set};

use super::{PackageSets, repo};

#[derive(Error, Debug)]
pub enum ListError {
    #[error("Unable to initialize repo state.")]
    RepoStateInitialization(Vec<repo::StateInitializationError>),

    #[error("Sets needed by the profile are missing from the repo.")]
    MissingSets(Vec<set::SetName>),
}

#[derive(Debug)]
pub struct ListSuccess {
    pub by_set: Vec<(set::SetName, Vec<String>)>,
    pub merged: Vec<String>,
}

pub fn list(profile: &MonjaProfile) -> Result<ListSuccess, ListError> {
    let repo = repo::initialize_state(profile).map_err(ListError::RepoStateInitialization)?;

    let PackageSets { by_set, merged } =
        super::gather(profile, &repo).map_err(ListError::MissingSets)?;

    Ok(ListSuccess { by_set, merged })
}
