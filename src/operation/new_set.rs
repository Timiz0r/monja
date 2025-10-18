use std::path::PathBuf;

use thiserror::Error;

use crate::{
    AbsolutePath, ExecutionOptions, LocalFilePath, MonjaProfile, MonjaProfileConfig,
    MonjaProfileConfigError, SetName, operation, repo,
};

#[derive(Error, Debug)]
pub enum NewSetError {
    #[error("Unable to add new set to profile.")]
    ProfileModification(SetName, #[source] MonjaProfileConfigError),

    #[error("Failed to create new set.")]
    SetCreation(#[from] repo::SetCreationError),

    #[error("Failed to configure the set's shortcut.")]
    SetShortcut(SetName, PathBuf, repo::SetConfigError),

    #[error("The put operation to place files in the new set failed.")]
    PutFiles(#[from] operation::put::PutError),
}

#[derive(Debug)]
pub struct NewSetSuccess {
    pub new_set: SetName,
    pub files: Vec<LocalFilePath>,
}

pub fn new_set(
    profile: &MonjaProfile,
    opts: &ExecutionOptions,
    profile_config_path: &AbsolutePath,
    files: Vec<LocalFilePath>,
    new_set: SetName,
    // boxing error because large, according to clippy
) -> Result<NewSetSuccess, Box<NewSetError>> {
    if opts.dry_run {
        return Ok(NewSetSuccess { new_set, files });
    }

    repo::create_empty_set(profile, &new_set).map_err(|e| Box::new(e.into()))?;

    let mut profile_config = MonjaProfileConfig::load(profile_config_path)
        .map_err(|e| NewSetError::ProfileModification(new_set.clone(), e))?;
    profile_config.target_sets.push(new_set.clone());
    profile_config
        .save(profile_config_path)
        .map_err(|e| NewSetError::ProfileModification(new_set.clone(), e))?;

    let shortcut = compute_shortcut(&files);
    let mut set_config = repo::SetConfig::load(profile, &new_set)
        .map_err(|e| NewSetError::SetShortcut(new_set.clone(), shortcut.clone(), e))?;
    set_config.shortcut = Some(shortcut.clone());
    set_config
        .save(profile, &new_set)
        .map_err(|e| NewSetError::SetShortcut(new_set.clone(), shortcut, e))?;

    // note that this wouldn't work in a dry run because the set isn't created, causing put to fail
    // updating the index is safe because, by putting the set last, it'll become the set that gets synced
    // it's also preferred that the user be able to modify and push immediately without pulling first
    let put_result =
        operation::put::put(profile, opts, files, new_set, true).map_err(|e| Box::new(e.into()))?;

    Ok(NewSetSuccess {
        new_set: put_result.owning_set,
        files: put_result.files,
    })
}

fn compute_shortcut(files: &[LocalFilePath]) -> PathBuf {
    if files.is_empty() {
        return PathBuf::new();
    }

    let mut prefix = PathBuf::new();
    let mut files: Vec<std::path::Components> = files.iter().map(|p| p.components()).collect();
    loop {
        let mut set = files.iter_mut().filter_map(|it| it.next());
        let Some(component) = set.next() else {
            break;
        };
        if set.all(|f| f == component) {
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
}
