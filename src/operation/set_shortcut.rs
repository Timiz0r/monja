use std::{fs, path::PathBuf};

use thiserror::Error;

use crate::{ExecutionOptions, MonjaProfile, repo};

#[derive(Error, Debug)]
pub enum SetShortcutError {
    #[error("Unable to initialize repo state.")]
    RepoStateInitialization(Vec<repo::StateInitializationError>),

    #[error("Set not found in repo.")]
    SetNotFound(repo::SetName),

    #[error("New shortcut is invalid.")]
    InvalidShortcut(#[from] repo::SetShortcutError),

    #[error(
        "File '{local_path}' would fall outside of the new shortcut '{new_shortcut}' (currently at '{current_path_in_set}' in set)"
    )]
    FileOutsideNewShortcut {
        local_path: PathBuf,
        new_shortcut: PathBuf,
        current_path_in_set: PathBuf,
    },

    #[error("Failed to create directory '{0}' in set.")]
    CreateDir(PathBuf, #[source] std::io::Error),

    #[error("Failed to move file from '{from}' to '{to}'.")]
    MoveFile {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to save set config.")]
    SaveConfig(#[from] repo::SetConfigError),

    #[error("Failed to clean up empty directories.")]
    Cleanup(PathBuf, #[source] walkdir::Error),
}

#[derive(Debug)]
pub struct SetShortcutSuccess {
    pub set_name: repo::SetName,
    pub old_shortcut: PathBuf,
    pub new_shortcut: PathBuf,
    pub files_moved: Vec<PathBuf>,
}

// choosing not to implement this in a transactional way because git will likely be used.
// to potential contributors: feel free to make the change though!
pub fn set_shortcut(
    profile: &MonjaProfile,
    opts: &ExecutionOptions,
    set_name: repo::SetName,
    new_shortcut: PathBuf,
) -> Result<SetShortcutSuccess, SetShortcutError> {
    let new_shortcut = repo::SetShortcut::from_path(new_shortcut)?;

    let repo =
        repo::initialize_full_state(profile).map_err(SetShortcutError::RepoStateInitialization)?;

    let set = repo
        .sets
        .get(&set_name)
        .ok_or_else(|| SetShortcutError::SetNotFound(set_name.clone()))?;

    // compute new paths for all files and validate they all fit under the new shortcut
    let mut moves: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut files_moved: Vec<PathBuf> = Vec::new();
    let new_shortcut_path = new_shortcut.to_path("");
    for file in set.locally_mapped_files.values() {
        let new_relative = new_shortcut.relative(file.path.local_path.as_ref());

        // check the new relative path doesn't escape the set
        if let Some(relative_path::Component::ParentDir) | None = new_relative.components().next() {
            return Err(SetShortcutError::FileOutsideNewShortcut {
                local_path: file.path.local_path.as_ref().to_path(""),
                new_shortcut: new_shortcut_path.clone(),
                current_path_in_set: file.path.path_in_set.to_path(""),
            });
        }

        let old_abs = file.path.path_in_set.to_path(&set.root);
        let new_abs = new_relative.to_path(&set.root);

        if old_abs != new_abs {
            moves.push((old_abs, new_abs));
            files_moved.push(new_relative.to_path(""));
        }
    }

    let old_shortcut = set.shortcut.to_path("");

    if opts.dry_run {
        return Ok(SetShortcutSuccess {
            set_name,
            old_shortcut,
            new_shortcut: new_shortcut_path,
            files_moved,
        });
    }

    for (from, to) in moves.iter() {
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| SetShortcutError::CreateDir(parent.to_path_buf(), e))?;
        }
        fs::rename(from, to).map_err(|e| SetShortcutError::MoveFile {
            from: from.clone(),
            to: to.clone(),
            source: e,
        })?;
    }

    cleanup_empty_dirs(&set.root)?;

    let mut config = repo::SetConfig::load(profile, &set_name)?;
    config.shortcut = if new_shortcut_path.as_os_str().is_empty() {
        None
    } else {
        Some(new_shortcut_path.clone())
    };
    config.save(profile, &set_name)?;

    Ok(SetShortcutSuccess {
        set_name,
        old_shortcut,
        new_shortcut: new_shortcut_path,
        files_moved,
    })
}

fn cleanup_empty_dirs(root: &std::path::Path) -> Result<(), SetShortcutError> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for entry in walkdir::WalkDir::new(root).min_depth(1) {
        let entry = entry.map_err(|e| SetShortcutError::Cleanup(root.to_path_buf(), e))?;
        if entry.file_type().is_dir() {
            dirs.push(entry.into_path());
        }
    }

    // sort by path length descending so we process deepest first
    // works since a child path is longer than its parent
    dirs.sort_by_key(|b| std::cmp::Reverse(b.as_os_str().len()));
    for dir in dirs {
        let _ = fs::remove_dir(&dir);
    }
    Ok(())
}
