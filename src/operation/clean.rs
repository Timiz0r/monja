use std::fs;

use thiserror::Error;

use crate::{
    ExecutionOptions, LocalFilePath, MonjaProfile,
    local::{self, FileIndexError},
    repo,
};

#[derive(Error, Debug)]
pub enum CleanError {
    #[error("Unable to initialize local state.")]
    LocalStateInitialization(#[from] local::StateInitializationError),
    #[error("Unable to initialize repo state.")]
    RepoStateInitialization(Vec<repo::StateInitializationError>),
    #[error("Failed to remove file.")]
    RemoveFile(#[source] std::io::Error),
    #[error("Unable to load an index file.")]
    FileIndex(#[from] FileIndexError),
}

#[derive(Debug)]
pub struct CleanSuccess {
    pub files_cleaned: Vec<LocalFilePath>,
}

pub enum CleanMode {
    Index,
    Full,
}

pub fn clean(
    profile: &MonjaProfile,
    opts: &ExecutionOptions,
    mode: CleanMode,
) -> Result<CleanSuccess, CleanError> {
    match mode {
        CleanMode::Index => index_clean(profile, opts),
        CleanMode::Full => full_clean(profile, opts),
    }
}

fn index_clean(
    profile: &MonjaProfile,
    opts: &ExecutionOptions,
) -> Result<CleanSuccess, CleanError> {
    let files_to_clean = local::old_files_since_last_pull(profile)?;

    if !opts.dry_run {
        for file in files_to_clean.iter() {
            let path = file.as_ref().to_path(&profile.local_root);
            fs::remove_file(path).map_err(CleanError::RemoveFile)?;
        }
    }

    let files_cleaned = files_to_clean.into_iter().map(|f| f.into()).collect();
    Ok(CleanSuccess { files_cleaned })
}

fn full_clean(profile: &MonjaProfile, opts: &ExecutionOptions) -> Result<CleanSuccess, CleanError> {
    let repo = repo::initialize_full_state(profile).map_err(CleanError::RepoStateInitialization)?;

    let local_state = local::retrieve_state(profile, &repo)?;

    let mut files_cleaned = Vec::with_capacity(
        local_state.missing_files.len()
            + local_state.files_with_missing_sets.len()
            + local_state.untracked_files.len(),
    );

    let files_to_clean = local_state
        .untracked_files
        .into_iter()
        .chain(local_state.files_with_missing_sets.into_values().flatten())
        .chain(local_state.missing_files.into_values().flatten());
    for file in files_to_clean {
        let path = file.as_ref().to_path(&profile.local_root);

        if !opts.dry_run {
            fs::remove_file(path).map_err(CleanError::RemoveFile)?;
        }

        files_cleaned.push(file.into());
    }

    // deref coercion to Path
    files_cleaned.sort_by(|l: &LocalFilePath, r: &LocalFilePath| l.cmp(r));
    Ok(CleanSuccess { files_cleaned })
}
