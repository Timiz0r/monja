use std::{collections::BTreeMap, fmt::Display, ops::Deref, path::PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{AbsolutePath, AbsolutePathError, MonjaProfileConfig};

#[derive(PartialEq, Eq, Hash, Clone, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RepoName(pub String);

impl RepoName {
    // the name a repo gets when the user didn't pick one: both the implicit name for the
    // single-repo `repo-dir` form (which predates repos being named at all) and the name
    // `monja init` gives the repo it creates.
    pub const DEFAULT: &'static str = "default";

    pub fn default_name() -> RepoName {
        RepoName(RepoName::DEFAULT.to_string())
    }
}

impl Display for RepoName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for RepoName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> AsRef<T> for RepoName
where
    T: ?Sized,
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}

impl From<&str> for RepoName {
    fn from(value: &str) -> Self {
        RepoName(value.to_string())
    }
}

#[derive(Error, Debug)]
pub enum ProfileError {
    #[error(
        "The profile specifies both 'repo-dir' and 'repos'. Use one or the other: 'repos' is the multi-repo form, and 'repo-dir' is the older single-repo form."
    )]
    ConflictingRepoConfig,

    #[error("The profile does not specify any repos. Add a 'repos' table (or a 'repo-dir').")]
    NoReposConfigured,

    #[error("The profile's 'default-repo' of '{name}' is not a configured repo. Available: {}", format_repo_names(.available))]
    UnknownDefaultRepo {
        name: RepoName,
        available: Vec<RepoName>,
    },

    #[error("Unable to resolve the directory of repo '{0}'.")]
    RepoPath(RepoName, #[source] AbsolutePathError),
}

#[derive(Error, Debug)]
pub enum RepoSelectionError {
    #[error("There is no repo named '{name}'. Available: {}", format_repo_names(.available))]
    UnknownRepo {
        name: RepoName,
        available: Vec<RepoName>,
    },

    #[error(
        "The profile has multiple repos, so one must be chosen with '--repo' (or by setting 'default-repo' in the profile). Available: {}",
        format_repo_names(.available)
    )]
    NoDefault { available: Vec<RepoName> },
}

pub(crate) fn format_repo_names(names: &[RepoName]) -> String {
    names
        .iter()
        .map(|n| format!("'{}'", n))
        .collect::<Vec<_>>()
        .join(", ")
}

// resolves the configured repo directories -- of either config form -- into absolute paths,
// erroring on the configurations that can't mean anything sensible.
pub(crate) fn resolve_repos(
    config: &MonjaProfileConfig,
    local_root: &AbsolutePath,
) -> Result<BTreeMap<RepoName, AbsolutePath>, ProfileError> {
    let configured: Vec<(RepoName, &PathBuf)> =
        match (config.repo_dir.as_ref(), config.repos.is_empty()) {
            (Some(_), false) => return Err(ProfileError::ConflictingRepoConfig),
            (None, true) => return Err(ProfileError::NoReposConfigured),
            (Some(dir), true) => vec![(RepoName::default_name(), dir)],
            (None, false) => config
                .repos
                .iter()
                .map(|(name, dir)| (name.clone(), dir))
                .collect(),
        };

    let mut resolved = BTreeMap::new();
    for (name, dir) in configured {
        let path = match dir.is_relative() {
            true => AbsolutePath::for_existing_path(&local_root.join(dir)),
            false => AbsolutePath::for_existing_path(dir),
        }
        .map_err(|e| ProfileError::RepoPath(name.clone(), e))?;

        resolved.insert(name, path);
    }

    if let Some(default_repo) = config.default_repo.as_ref()
        && !resolved.contains_key(default_repo)
    {
        return Err(ProfileError::UnknownDefaultRepo {
            name: default_repo.clone(),
            available: resolved.into_keys().collect(),
        });
    }

    Ok(resolved)
}
