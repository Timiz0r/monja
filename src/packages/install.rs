use thiserror::Error;

use crate::{ExecutionOptions, MonjaProfile, set};

use super::{PackageSets, repo};

#[derive(Error, Debug)]
pub enum InstallError {
    #[error("Unable to initialize repo state.")]
    RepoStateInitialization(Vec<repo::StateInitializationError>),

    #[error("Sets needed by the profile are missing from the repo.")]
    MissingSets(Vec<set::SetName>),
}

#[derive(Debug)]
pub struct InstallSuccess {
    pub packages: Vec<String>,
}

pub fn install(
    profile: &MonjaProfile,
    opts: &ExecutionOptions,
) -> Result<InstallSuccess, InstallError> {
    let repo = repo::initialize_state(profile).map_err(InstallError::RepoStateInitialization)?;

    let PackageSets { merged, .. } =
        super::gather(profile, &repo).map_err(InstallError::MissingSets)?;

    if !opts.dry_run {
        dispatch_to_package_manager(&merged);
    }

    Ok(InstallSuccess { packages: merged })
}

// stub: no package manager is actually invoked yet.
// this is where dispatch to a real package manager (apt, pacman, brew, etc.) would go.
fn dispatch_to_package_manager(_packages: &[String]) {}
