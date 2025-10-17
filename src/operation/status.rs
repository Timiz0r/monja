use thiserror::Error;

use crate::{LocalFilePath, MonjaProfile, convert_set_localfile_result, local, repo};

#[derive(Error, Debug)]
pub enum StatusError {
    #[error("Unable to initialize repo state.")]
    RepoStateInitialization(Vec<repo::StateInitializationError>),

    #[error("Unable to initialize local state.")]
    LocalStateInitialization(#[from] local::StateInitializationError),

    #[error("Unable to parse location.")]
    Location(LocalFilePath),
}

#[derive(Debug)]
pub struct Status {
    pub files_to_push: Vec<(repo::SetName, Vec<LocalFilePath>)>,
    pub files_with_missing_sets: Vec<(repo::SetName, Vec<LocalFilePath>)>,
    pub missing_files: Vec<(repo::SetName, Vec<LocalFilePath>)>,
    pub untracked_files: Vec<LocalFilePath>,
    pub old_files_after_last_pull: Vec<LocalFilePath>,
}

pub fn local_status(
    profile: &MonjaProfile,
    location: LocalFilePath,
) -> Result<Status, StatusError> {
    let repo =
        repo::initialize_full_state(profile).map_err(StatusError::RepoStateInitialization)?;
    let local_state = local::retrieve_state(profile, &repo)?;
    // only cloning in case error. but it's just one clone so cheap enough.
    let location = location.to_internal();

    let files_to_push = convert_set_localfile_result(
        &profile.config.target_sets,
        local_state.files_to_push,
        &location,
    );

    let files_with_missing_sets = convert_set_localfile_result(
        &profile.config.target_sets,
        local_state.files_with_missing_sets,
        &location,
    );

    let missing_files = convert_set_localfile_result(
        &profile.config.target_sets,
        local_state.missing_files,
        &location,
    );

    let old_files_after_last_pull = local_state
        .old_files_since_last_pull
        .into_iter()
        .filter(|p: &local::FilePath| p.is_child_of(&location))
        .map(|f| f.into())
        .collect();

    let untracked_files = local_state
        .untracked_files
        .into_iter()
        .filter(|p: &local::FilePath| p.is_child_of(&location))
        .map(|f| f.into())
        .collect();

    Ok(Status {
        files_to_push,
        files_with_missing_sets,
        missing_files,
        old_files_after_last_pull,
        untracked_files,
    })
}
