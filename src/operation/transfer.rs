use std::{fs, path::PathBuf};

use thiserror::Error;

use crate::{
    ExecutionOptions, LocalFilePath, MonjaProfile, local,
    repo::{self, SetPathError},
};

#[derive(Error, Debug)]
pub enum TransferError {
    #[error("Unable to initialize repo state.")]
    RepoStateInitialization(Vec<repo::StateInitializationError>),

    #[error("Source set not found in repo.")]
    SourceSetNotFound(repo::SetName),

    #[error("Destination set not found in repo.")]
    DestSetNotFound(repo::SetName),

    #[error("Failed to load monja-index.toml.")]
    FileIndex(#[from] local::FileIndexError),

    #[error("Failed to copy local file to destination set.")]
    CopyToDest {
        set_name: repo::SetName,
        local_path: PathBuf,
        repo_path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to create the directory in the destination set.")]
    CreateDestDir(PathBuf, #[source] std::io::Error),

    #[error("Either path isn't a file, or the directory could not be extracted from the path.")]
    NotValidFile(PathBuf),

    #[error("Unable to formulate the path as it would be in the destination set folder.")]
    DestSetPath(#[from] SetPathError),

    #[error("Failed to remove file from source set.")]
    RemoveFromSource {
        set_name: repo::SetName,
        repo_path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("File is not tracked by the source set.")]
    NotInSourceSet {
        set_name: repo::SetName,
        local_path: LocalFilePath,
    },
}

#[derive(Debug)]
pub struct TransferSuccess {
    pub source_set: repo::SetName,
    pub dest_set: repo::SetName,
    pub files: Vec<LocalFilePath>,
}

pub fn transfer(
    profile: &MonjaProfile,
    opts: &ExecutionOptions,
    files: Vec<LocalFilePath>,
    source_set: repo::SetName,
    dest_set: repo::SetName,
) -> Result<TransferSuccess, TransferError> {
    let repo =
        repo::initialize_full_state(profile).map_err(TransferError::RepoStateInitialization)?;
    let mut index = local::FileIndex::load(profile, local::IndexKind::Current)?;

    let source = repo
        .sets
        .get(&source_set)
        .ok_or_else(|| TransferError::SourceSetNotFound(source_set.clone()))?;

    let dest = repo
        .sets
        .get(&dest_set)
        .ok_or_else(|| TransferError::DestSetNotFound(dest_set.clone()))?;

    let dest_set_pos = profile
        .config
        .target_sets
        .iter()
        .position(|s| *s == dest_set);

    let mut result_files = Vec::with_capacity(files.len());
    for path in files.into_iter() {
        let internal_path = path.to_internal();

        if !source.tracks_file(&internal_path) {
            return Err(TransferError::NotInSourceSet {
                set_name: source_set.clone(),
                local_path: path,
            });
        }

        // validate destination path before doing anything
        dest.get_repo_absolute_path_for(&internal_path)?;

        if !opts.dry_run {
            copy_to_dest(profile, dest, &internal_path)?;
            remove_from_source(source, &internal_path)?;
        }

        // update the index if the dest set is the latest set for this file
        let mut dest_is_latest = true;
        for (set_name, set) in repo.sets.iter() {
            if *set_name == dest_set || *set_name == source_set {
                continue;
            }
            if !set.tracks_file(&internal_path) {
                continue;
            }
            let curr_pos = profile
                .config
                .target_sets
                .iter()
                .position(|s| s == set_name);
            if curr_pos > dest_set_pos {
                dest_is_latest = false;
                break;
            }
        }

        if dest_is_latest {
            index.set(internal_path, dest_set.clone());
        }

        result_files.push(path);
    }

    if !opts.dry_run {
        index.save(profile, local::IndexKind::Current)?;
    }

    Ok(TransferSuccess {
        source_set,
        dest_set,
        files: result_files,
    })
}

fn copy_to_dest(
    profile: &MonjaProfile,
    dest: &repo::Set,
    path: &local::FilePath,
) -> Result<(), TransferError> {
    let copy_from = path.to_absolute_path(profile);
    if !copy_from.is_file() {
        return Err(TransferError::NotValidFile(copy_from));
    }
    let copy_to = dest.get_repo_absolute_path_for(path)?;
    let copy_to_dir = copy_to
        .parent()
        .ok_or_else(|| TransferError::NotValidFile(copy_to.to_path_buf()))?;

    fs::create_dir_all(copy_to_dir)
        .map_err(|e| TransferError::CreateDestDir(copy_to_dir.to_path_buf(), e))?;

    fs::copy(&copy_from, &copy_to).map_err(|e| TransferError::CopyToDest {
        set_name: dest.name.clone(),
        local_path: copy_from,
        repo_path: copy_to,
        source: e,
    })?;

    Ok(())
}

fn remove_from_source(source: &repo::Set, path: &local::FilePath) -> Result<(), TransferError> {
    let repo_path = source.get_repo_absolute_path_for(path)?;

    fs::remove_file(&repo_path).map_err(|e| TransferError::RemoveFromSource {
        set_name: source.name.clone(),
        repo_path,
        source: e,
    })?;

    Ok(())
}
