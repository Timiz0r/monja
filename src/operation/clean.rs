use std::fs;

use thiserror::Error;

use crate::{ExecutionOptions, LocalFilePath, MonjaProfile, local, repo};

#[derive(Error, Debug)]
pub enum CleanError {
    #[error("Unable to initialize local state.")]
    LocalStateInitialization(#[from] local::StateInitializationError),
    #[error("Unable to initialize repo state.")]
    RepoStateInitialization(Vec<repo::StateInitializationError>),
    #[error("Failed to delete file.")]
    Io(#[source] std::io::Error),

    // data structure is internal implementation detail, so just go with this.
    #[error("Unable to read monja-index.toml.")]
    CurrentFileIndex,
    #[error("Unable to read monja-index-prev.toml.")]
    PreviousFileIndex,
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
    let files_to_clean = local::old_files_since_last_pull(profile).map_err(convert_index_error)?;

    if !opts.dry_run {
        for file in files_to_clean.iter() {
            let path = file.as_ref().to_path(&profile.local_root);
            fs::remove_file(path).map_err(CleanError::Io)?;
        }
    }

    let files_cleaned = files_to_clean.into_iter().map(|f| f.into()).collect();
    Ok(CleanSuccess { files_cleaned })
}

fn full_clean(profile: &MonjaProfile, opts: &ExecutionOptions) -> Result<CleanSuccess, CleanError> {
    let repo = repo::initialize_full_state(profile).map_err(CleanError::RepoStateInitialization)?;

    let local_state = local::retrieve_state(profile, &repo).map_err(|e| match e {
        local::StateInitializationError::FileIndex(file_index_error) => {
            convert_index_error(file_index_error)
        }
        e => e.into(),
    })?;

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
            fs::remove_file(path).map_err(CleanError::Io)?;
        }

        files_cleaned.push(file.into());
    }

    // deref coercion to Path
    files_cleaned.sort_by(|l: &LocalFilePath, r: &LocalFilePath| l.cmp(r));
    Ok(CleanSuccess { files_cleaned })
}

fn convert_index_error(e: local::FileIndexError) -> CleanError {
    use local::IndexKind::*;
    match e {
        local::FileIndexError::Io(Current, _) => CleanError::CurrentFileIndex,
        local::FileIndexError::Io(Previous, _) => CleanError::PreviousFileIndex,

        local::FileIndexError::Deserialization(Current, _) => CleanError::CurrentFileIndex,
        local::FileIndexError::Deserialization(Previous, _) => CleanError::PreviousFileIndex,

        local::FileIndexError::Serialization(Current, _) => CleanError::CurrentFileIndex,
        local::FileIndexError::Serialization(Previous, _) => CleanError::PreviousFileIndex,
    }
}
