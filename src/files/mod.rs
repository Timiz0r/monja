use std::{collections::HashMap, path::PathBuf};

use crate::set;

pub mod clean;
pub mod pull;
pub mod push;
pub mod put;
pub mod set_shortcut;
pub mod status;
pub mod transfer;

pub(crate) mod local;
pub(crate) mod repo;
pub(crate) mod rsync;

#[derive(Debug, PartialEq, Eq)]
pub struct RepoFilePath {
    pub path_in_set: PathBuf,
    pub local_path: PathBuf,
}

impl From<repo::FilePath> for RepoFilePath {
    fn from(value: repo::FilePath) -> Self {
        RepoFilePath {
            path_in_set: value.path_in_set.to_path(""),
            local_path: value.local_path.into(),
        }
    }
}

impl TryFrom<RepoFilePath> for repo::FilePath {
    type Error = relative_path::FromPathError;

    fn try_from(value: RepoFilePath) -> Result<Self, Self::Error> {
        Ok(repo::FilePath {
            path_in_set: relative_path::RelativePathBuf::from_path(&value.path_in_set)?,
            local_path: value.local_path.try_into()?,
        })
    }
}

// want to keep local::FilePath/repo::File internal, so gonna bite the bullet on allocating another vector.
// this is mainly to avoid exporting RelativePath(Buf).
pub(crate) fn convert_set_localfile_result(
    // we use these sets to keep the ordering nice
    set_names: &[set::SetName],
    mut source: HashMap<set::SetName, Vec<local::FilePath>>,
    location: &local::FilePath,
) -> Vec<(set::SetName, Vec<crate::LocalFilePath>)> {
    let mut result = Vec::with_capacity(source.len());

    result.extend(
        set_names
            .iter()
            .filter_map(|name| source.remove_entry(name))
            .map(|(name, set)| {
                (
                    name,
                    set.into_iter()
                        .filter(|p: &local::FilePath| p.is_child_of(location))
                        .map(|p| p.into())
                        .collect(),
                )
            }),
    );

    result
}

pub(crate) fn convert_set_repofile_result(
    // we use these sets to keep the ordering nice
    set_names: &[set::SetName],
    mut source: HashMap<set::SetName, Vec<repo::FilePath>>,
) -> Vec<(set::SetName, Vec<RepoFilePath>)> {
    let mut result = Vec::with_capacity(source.len());

    result.extend(
        set_names
            .iter()
            .filter_map(|name| source.remove_entry(name))
            .map(|(name, set)| (name, set.into_iter().map(|p| p.into()).collect())),
    );

    result
}
