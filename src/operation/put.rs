use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
};

use thiserror::Error;

use crate::{
    ExecutionOptions, LocalFilePath, MonjaProfile, SetName, local,
    repo::{self, SetPathError},
};

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

    #[error("Unable to formulate the path as it would be in the set folder.")]
    SetPath(#[from] SetPathError),
}

#[derive(Debug)]
pub struct PutSuccess {
    pub owning_set: repo::SetName,
    pub files: Vec<LocalFilePath>,

    pub set_is_targeted: bool,
    pub files_in_later_sets: Vec<(LocalFilePath, Vec<repo::SetName>)>,
    pub untracked_files: Vec<LocalFilePath>,
}

pub fn put(
    profile: &MonjaProfile,
    opts: &ExecutionOptions,
    files: Vec<LocalFilePath>,
    owning_set: repo::SetName,
    update_index: bool,
) -> Result<PutSuccess, PutError> {
    let repo = repo::initialize_full_state(profile).map_err(PutError::RepoStateInitialization)?;
    let mut index = match update_index {
        true => local::FileIndex::load(profile, local::IndexKind::Current)?,
        // will also be unused. mainly just saving time not having to load
        false => local::FileIndex::new(),
    };

    let set = repo
        .sets
        .get(&owning_set)
        .ok_or_else(|| PutError::SetNotFound(owning_set.clone()))?;

    let owning_set_pos = profile
        .config
        .target_sets
        .iter()
        .position(|s: &SetName| *s == owning_set);

    let mut tracked_files: HashSet<LocalFilePath> = HashSet::new();
    let mut files_in_later_sets: HashMap<LocalFilePath, Vec<repo::SetName>> = HashMap::new();
    let mut result_files = Vec::with_capacity(files.len());
    for path in files.into_iter() {
        let internal_path: local::FilePath = match path.clone().try_into() {
            Ok(path) => path,
            Err(_) => return Err(PutError::PathParse(path)),
        };

        let copy_from = internal_path.to_path(&profile.local_root);
        if !copy_from.is_file() {
            return Err(PutError::NotValidFile(copy_from.to_path_buf()));
        }
        let copy_to = set.get_repo_absolute_path_for(&internal_path)?;

        let copy_to_dir = copy_to
            .parent()
            .ok_or_else(|| PutError::NotValidFile(copy_to.to_path_buf()))?;
        if !opts.dry_run {
            fs::create_dir_all(copy_to_dir)
                .map_err(|e| PutError::CreateDestDir(copy_to_dir.to_path_buf(), e))?;
        }

        if !opts.dry_run {
            fs::copy(&copy_from, &copy_to).map_err(|e| PutError::CopyToSet {
                set_name: owning_set.clone(),
                local_path: copy_from,
                repo_path: copy_to,
                source: e,
            })?;
        }

        for (set_name, set) in repo.sets.iter() {
            let is_dest_set = owning_set_pos.is_some() && owning_set == *set_name;
            // the sets here don't reflect the fact that we're pushing files to
            if !is_dest_set && !set.tracks_file(&internal_path) {
                continue;
            }

            // checking contains first to avoid extra clones
            if !tracked_files.contains(&path) {
                tracked_files.insert(path.clone());
            }

            let curr_pos: Option<usize> = profile
                .config
                .target_sets
                .iter()
                .position(|s: &SetName| s == set_name);
            if curr_pos > owning_set_pos {
                // we do an extra get_mut, instead of just using entry, to avoid extra clones of path
                match files_in_later_sets.get_mut(&path) {
                    Some(sets) => sets.push(set_name.clone()),
                    None => {
                        files_in_later_sets
                            .entry(path.clone())
                            .or_default()
                            .push(set_name.clone());
                    }
                };
            }
        }

        result_files.push(path);

        // updating the index allows the put command to fix issues that happen
        // when the repo is changed in a way that removes files, followed by an attempted push
        if update_index {
            index.set(internal_path, owning_set.clone());
        }
    }

    if update_index && !opts.dry_run {
        index.save(profile, local::IndexKind::Current)?;
    }

    let untracked_files = result_files
        .iter()
        .filter(|p| !tracked_files.contains(p))
        .cloned()
        .collect();
    Ok(PutSuccess {
        owning_set: owning_set.clone(),
        files: result_files,
        set_is_targeted: owning_set_pos.is_some(),
        files_in_later_sets: files_in_later_sets
            .into_iter()
            .map(|(path, sets)| (path.clone(), sets))
            .collect(),
        untracked_files,
    })
}
