use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::{
    AbsolutePath, ExecutionOptions, LocalFilePath, MonjaProfile, MonjaProfileConfig,
    MonjaProfileConfigError, RepoName, RepoSelectionError, SetName, files, set,
};

#[derive(Error, Debug)]
pub enum NewSetError {
    #[error("Unable to determine which repo to create the set in.")]
    RepoSelection(#[from] RepoSelectionError),

    #[error("Unable to add new set to profile.")]
    ProfileModification(SetName, #[source] MonjaProfileConfigError),

    #[error("Failed to create new set.")]
    SetCreation(#[from] set::SetCreationError),

    #[error("Failed to configure the set's shortcut.")]
    SetShortcut(SetName, PathBuf, set::SetConfigError),

    #[error("The put operation to place files in the new set failed.")]
    PutFiles(#[from] files::put::PutError),
}

#[derive(Debug)]
pub struct NewSetSuccess {
    pub new_set: SetName,
    pub repo: RepoName,
    pub files: Vec<LocalFilePath>,
    pub added_to_profile: bool,
}

pub fn new_set(
    profile: &MonjaProfile,
    opts: &ExecutionOptions,
    // None leaves the profile alone, for sets that shouldn't be targeted -- say, one meant for
    // another machine.
    profile_config_path: Option<&AbsolutePath>,
    files: Vec<LocalFilePath>,
    new_set: SetName,
    // which repo to create the set in. resolved here, rather than by the caller, so the
    // requested -> default-repo -> sole-repo chain lives in one place.
    repo: Option<RepoName>,
    // boxing error because large, according to clippy
) -> Result<NewSetSuccess, Box<NewSetError>> {
    let (repo_name, repo_root) = profile
        .resolve_repo(repo.as_ref())
        .map_err(|e| Box::new(NewSetError::from(e)))?;
    let repo_name = repo_name.clone();

    if opts.dry_run {
        return Ok(NewSetSuccess {
            new_set,
            repo: repo_name,
            files,
            added_to_profile: profile_config_path.is_some(),
        });
    }

    let set_root =
        set::create_empty_set(profile, repo_root, &new_set).map_err(|e| Box::new(e.into()))?;

    if let Some(profile_config_path) = profile_config_path {
        let mut profile_config = MonjaProfileConfig::load(profile_config_path)
            .map_err(|e| NewSetError::ProfileModification(new_set.clone(), e))?;
        profile_config.target_sets.push(new_set.clone());
        profile_config
            .save(profile_config_path)
            .map_err(|e| NewSetError::ProfileModification(new_set.clone(), e))?;
    }

    // an empty shortcut is nothing to configure, and skipping the save keeps the sample config's
    // comments around -- which is the whole point for a set created without files.
    let shortcut = compute_shortcut(&files);
    if !shortcut.as_os_str().is_empty() {
        let mut set_config = set::SetConfig::load(&set_root, &new_set)
            .map_err(|e| NewSetError::SetShortcut(new_set.clone(), shortcut.clone(), e))?;
        set_config.shortcut = Some(shortcut.clone());
        set_config
            .save(&set_root, &new_set)
            .map_err(|e| NewSetError::SetShortcut(new_set.clone(), shortcut, e))?;
    }

    // note that this wouldn't work in a dry run because the set isn't created, causing put to fail
    let put_result =
        files::put::put(profile, opts, files, new_set).map_err(|e| Box::new(e.into()))?;

    Ok(NewSetSuccess {
        new_set: put_result.owning_set,
        repo: repo_name,
        files: put_result.files,
        added_to_profile: profile_config_path.is_some(),
    })
}

fn compute_shortcut(files: &[LocalFilePath]) -> PathBuf {
    // the shortcut is the deepest folder containing every file, so file names don't participate:
    // one file's shortcut is its folder, not the file itself.
    let mut dirs: Vec<std::path::Components> = files
        .iter()
        .map(|p| p.parent().unwrap_or_else(|| Path::new("")).components())
        .collect();
    if dirs.is_empty() {
        return PathBuf::new();
    }

    let mut prefix = PathBuf::new();
    loop {
        // a file sitting at the current depth ends the prefix, since nothing deeper could contain it
        let mut components = dirs.iter_mut().map(|it| it.next());
        let Some(Some(component)) = components.next() else {
            break;
        };
        if components.all(|c| c == Some(component)) {
            prefix.push(component);
        } else {
            break;
        }
    }

    prefix
}

// unit testing compute_shortcut due to complexity. eligible to be deleted, since it gets covered in integration tests.
#[cfg(test)]
mod localfilepath_tests {
    use std::path::Path;

    use googletest::prelude::*;

    use crate::LocalFilePath;

    #[gtest]
    fn simple() -> Result<()> {
        let paths: [LocalFilePath; _] = [
            LocalFilePath("foo/bar/yay".into()),
            LocalFilePath("foo/bar/omg/bbq".into()),
            LocalFilePath("foo/bar/aaaaa/a/a/a/a/a/a".into()),
            LocalFilePath("foo/bar/aa/a/a".into()),
            LocalFilePath("foo/bar/a/a//a/aaa".into()),
            LocalFilePath("foo/bar/aaaa/a".into()),
        ];

        let shortcut = super::compute_shortcut(&paths);
        expect_that!(shortcut, eq(Path::new("foo/bar")));
        Ok(())
    }

    #[gtest]
    fn no_shortcut() -> Result<()> {
        let paths: [LocalFilePath; _] = [
            LocalFilePath("a/bar/yay".into()),
            LocalFilePath("b/bar/omg/bbq".into()),
            LocalFilePath("c/bar/aaaaa/a/a/a/a/a/a".into()),
            LocalFilePath("d/bar/aa/a/a".into()),
            LocalFilePath("e/bar/a/a//a/aaa".into()),
            LocalFilePath("f/bar/aaaa/a".into()),
        ];

        let shortcut = super::compute_shortcut(&paths);
        expect_that!(shortcut, eq(Path::new("")));
        Ok(())
    }

    #[gtest]
    fn single_file_uses_its_folder() -> Result<()> {
        let paths: [LocalFilePath; _] = [LocalFilePath("foo/bar/yay".into())];

        let shortcut = super::compute_shortcut(&paths);
        expect_that!(shortcut, eq(Path::new("foo/bar")));
        Ok(())
    }

    #[gtest]
    fn file_at_shallowest_folder_ends_the_shortcut() -> Result<()> {
        let paths: [LocalFilePath; _] = [
            LocalFilePath("foo/bar/yay".into()),
            LocalFilePath("foo/bar/omg/bbq".into()),
        ];

        let shortcut = super::compute_shortcut(&paths);
        expect_that!(shortcut, eq(Path::new("foo/bar")));
        Ok(())
    }

    #[gtest]
    fn no_files() -> Result<()> {
        let shortcut = super::compute_shortcut(&[]);
        expect_that!(shortcut, eq(Path::new("")));
        Ok(())
    }
}
