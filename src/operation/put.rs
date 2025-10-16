use std::{fs, path::PathBuf};

use thiserror::Error;

use crate::{ExecutionOptions, LocalFilePath, MonjaProfile, local, repo};

#[derive(Error, Debug)]
pub enum PutError {
    #[error("Unable to initialize repo state.")]
    RepoStateInitialization(Vec<repo::StateInitializationError>),
    #[error("Set not found in repo.")]
    SetNotFound(repo::SetName),

    #[error("Failed to load monja-index.toml.")]
    FileIndex(#[from] local::FileIndexError),

    // TODO: refine all of our Io errors
    #[error("Failed to copy local file to repo.")]
    CopyToSet {
        set_name: repo::SetName,
        local_path: PathBuf,
        repo_path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to create the directory in the set that the local file will be copied to.")]
    CreateDestDir(PathBuf, #[source] std::io::Error),

    #[error("Failed to parse the local file path.")]
    PathParse(LocalFilePath),

    #[error("Either path isn't a file, or the directory could not be extracted from the path.")]
    NotValidFile(PathBuf),
}

pub struct PutSuccess {
    pub owning_set: repo::SetName,
    pub files: Vec<LocalFilePath>,
}

pub fn put(
    profile: &MonjaProfile,
    opts: &ExecutionOptions,
    files: &[LocalFilePath],
    owning_set: repo::SetName,
) -> Result<PutSuccess, PutError> {
    let repo = repo::initialize_full_state(profile).map_err(PutError::RepoStateInitialization)?;
    let mut index = local::FileIndex::load(profile, local::IndexKind::Current)?;

    let mut result_files = Vec::with_capacity(files.len());
    for path in files {
        result_files.push(path.clone());

        let path: local::FilePath = path
            .try_into()
            .map_err(|_| PutError::PathParse(path.clone()))?;

        let copy_from = path.to_path(&profile.local_root);
        if !copy_from.is_file() {
            return Err(PutError::NotValidFile(copy_from.to_path_buf()));
        }

        let set = repo
            .sets
            .get(&owning_set)
            .ok_or_else(|| PutError::SetNotFound(owning_set.clone()))?;
        let copy_to = set.get_repo_absolute_path_for(&path);

        let copy_to_dir = copy_to
            .parent()
            .ok_or_else(|| PutError::NotValidFile(copy_to.to_path_buf()))?;
        if !opts.dry_run {
            fs::create_dir_all(copy_to_dir)
                .map_err(|e| PutError::CreateDestDir(copy_to_dir.to_path_buf(), e))?;
        }

        index.set(path, owning_set.clone());

        if !opts.dry_run {
            fs::copy(&copy_from, &copy_to).map_err(|e| PutError::CopyToSet {
                set_name: owning_set.clone(),
                local_path: copy_from,
                repo_path: copy_to,
                source: e,
            })?;
        }
    }

    if !opts.dry_run {
        index.save(profile, local::IndexKind::Current)?;
    }

    Ok(PutSuccess {
        owning_set: owning_set.clone(),
        files: result_files,
    })
}
