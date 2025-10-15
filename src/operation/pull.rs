use std::collections::HashMap;

use thiserror::Error;

use crate::{
    AbsolutePath, ExecutionOptions, LocalFilePath, MonjaProfile, RepoFilePath, SetName,
    convert_set_file_result, local, repo, rsync,
};

#[derive(Error, Debug)]
pub enum PullError {
    #[error("Unable to initialize repo state.")]
    RepoStateInitialization(Vec<repo::StateInitializationError>),

    #[error("Sets needed by the profile are missing from the repo.")]
    MissingSets(Vec<repo::SetName>),

    #[error("Failed to copy files via rsync.")]
    Rsync(#[source] std::io::Error),

    #[error("Unable to save file index.")]
    FileIndex(#[from] local::FileIndexError),

    #[error("Error when walking local files to find out which are ignored.")]
    LocalWalk(#[from] local::LocalWalkError),
}

#[derive(Debug)]
pub struct PullSuccess {
    pub files_pulled: Vec<(SetName, Vec<RepoFilePath>)>,

    pub cleanable_files: Vec<LocalFilePath>,
}

pub fn pull(profile: &MonjaProfile, opts: &ExecutionOptions) -> Result<PullSuccess, PullError> {
    let mut set_info = HashMap::with_capacity(profile.config.target_sets.len());

    let mut repo =
        repo::initialize_full_state(profile).map_err(PullError::RepoStateInitialization)?;
    // we first need a map on local path in order to pick the set associated with the file.
    // rsync, however, needs to be run per-set, so we'll group them later.
    let mut files: HashMap<local::FilePath, repo::File> = HashMap::new();

    let mut missing_sets = Vec::new();
    for set_name in profile.config.target_sets.iter() {
        if !repo.sets.contains_key(set_name) {
            missing_sets.push(set_name.clone());
            continue;
        };

        // if we find a missing set, save us the trouble of handling files
        if !missing_sets.is_empty() {
            continue;
        }

        let set = repo
            .sets
            .remove(set_name)
            .expect("We verified it existed where we aggregate missing sets.");
        set_info.insert(
            set_name,
            SetInfo {
                root: set.root,
                shortcut: set.shortcut,
            },
        );

        for (local_path, repo_file) in set.locally_mapped_files.into_iter() {
            files.insert(local_path, repo_file);
        }
    }
    // since we removed from the sets to get ownership of them, we want to move sets to ensure it doesn't get used.
    std::mem::drop(repo.sets);

    if !missing_sets.is_empty() {
        return Err(PullError::MissingSets(missing_sets));
    }

    let mut files_to_pull = HashMap::with_capacity(set_info.len());
    let mut updated_index = local::FileIndex::new();
    for (local_path, repo_file) in files.into_iter() {
        files_to_pull
            .entry(repo_file.owning_set.clone())
            .or_insert_with(Vec::new)
            .push(repo_file.path);

        // TODO: what if rsync failed and we don't update index even though some copies happened?
        updated_index.set(local_path, repo_file.owning_set);
    }

    if !opts.dry_run {
        for set_name in profile.config.target_sets.iter() {
            let Some(file_paths) = files_to_pull.get(set_name) else {
                // would happen if there are no files to pull for the set
                continue;
            };
            let set = set_info
                .get(set_name)
                .expect("Already checked for missing sets.");

            // lets say set shortcut is foo/bar and file baz
            // transfer looks something like this: /monja/set/baz -> /home/xx/foo/bar/baz
            // here, the source is /monja/set/, dest is /home/xx/foo/bar/, and file is baz
            // incidentally, local::FilePath is foo/bar/baz

            rsync(
                set.root.as_ref(),
                &set.shortcut.to_path(&profile.local_root),
                file_paths.iter().map(|p| p.path_in_set.to_path("")),
                opts,
            )
            .map_err(PullError::Rsync)?;
        }
    }

    let prev_index = local::FileIndex::load(profile, local::IndexKind::Current)?;
    if !opts.dry_run {
        updated_index.save(profile, local::IndexKind::Current)?;
        // could also hypothetically copy the file. in fact, it's technically better, but it doesn't really matter.
        prev_index.save(profile, local::IndexKind::Previous)?;
    }

    let files_pulled = convert_set_file_result(&profile.config.target_sets, files_to_pull);
    let cleanable_files = prev_index
        .into_files_not_in(profile, &updated_index)?
        .into_iter()
        .map(|f| f.into())
        .collect();
    return Ok(PullSuccess {
        files_pulled,
        cleanable_files,
    });

    // the code ends up being the cleanest when files takes ownership of its data from repo,
    // since that data becomes part of the result.
    // in order to take ownership, we .remove() them (from sets).
    // if we instead used get(), we'd have to do a lot of cloning.
    // the only problem is we partial move the set, so it's not like we can store it anywhere directly.
    // instead, we just move out the rest of the set info we need, at the cost of a small hashmap.
    struct SetInfo {
        root: AbsolutePath,
        shortcut: repo::SetShortcut,
    }
}
