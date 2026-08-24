use std::{
    collections::BTreeMap,
    fmt::Display,
    fs,
    ops::Deref,
    path::{Path, PathBuf},
};

use indoc::formatdoc;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    AbsolutePath, AbsolutePathError, MonjaProfileConfig, MonjaProfileConfigError, SetName,
};

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

// where a repo monja creates for itself lives, relative to the data root. a `repos/<name>`
// directory rather than a single `repo` one, so that adding a second repo later doesn't need a
// different layout than the first.
pub(crate) fn repo_root_for(data_root: &AbsolutePath, repo_name: &RepoName) -> PathBuf {
    data_root.join("repos").join(repo_name)
}

#[derive(Error, Debug)]
pub enum RegisterRepoError {
    #[error(
        "The profile uses the older single-repo 'repo-dir' form, which can't hold a second repo. Convert it to a '[repos]' table first."
    )]
    LegacyRepoDir,

    #[error("The profile already has a repo named '{0}'.")]
    RepoAlreadyConfigured(RepoName),

    #[error("Unable to read the existing profile.")]
    ProfileLoad(#[source] MonjaProfileConfigError),

    #[error("Unable to write the profile.")]
    ProfileWrite(#[source] MonjaProfileConfigError),

    #[error("Unable to create the profile.")]
    ProfileCreate(#[source] std::io::Error),
}

// whether registering a repo had to bring a profile into existence, so callers can report the
// difference between getting set up and merely gaining a repo.
#[derive(Debug, PartialEq, Eq)]
pub enum RepoRegistration {
    CreatedProfile,
    AddedToExisting,
}

// the checks `register_repo` would make, without any of its side effects. exists so that a
// command with an expensive, externally-visible step (cloning) can refuse a repo it wouldn't be
// able to register *before* performing that step.
pub(crate) fn validate_registration(
    profile_config_path: &Path,
    repo_name: &RepoName,
) -> Result<(), RegisterRepoError> {
    let Some(config) = load_existing(profile_config_path)? else {
        return Ok(());
    };

    validate_against(&config, repo_name)
}

// adds a repo to the profile, creating the profile if it doesn't exist yet.
//
// `initial_set` is only used for a profile being created: an existing profile's `target-sets`
// (and `default-repo`) are left exactly as the user had them, since adding a repo says nothing
// about whether its sets are wanted.
pub(crate) fn register_repo(
    profile_config_path: &Path,
    local_root: &AbsolutePath,
    repo_root: &AbsolutePath,
    repo_name: &RepoName,
    initial_set: Option<&SetName>,
) -> Result<RepoRegistration, RegisterRepoError> {
    // a path under the local root is recorded relative to it, so a profile stays portable
    // across machines whose home directories differ.
    let configured_dir = repo_root
        .strip_prefix(local_root)
        .unwrap_or(repo_root)
        .to_path_buf();

    let Some(mut config) = load_existing(profile_config_path)? else {
        create_profile(profile_config_path, &configured_dir, repo_name, initial_set)?;
        return Ok(RepoRegistration::CreatedProfile);
    };

    validate_against(&config, repo_name)?;

    config.repos.insert(repo_name.clone(), configured_dir);
    // the path has to exist for AbsolutePath, and it does by the time we're registering
    let profile_config_path = AbsolutePath::for_existing_path(profile_config_path)
        .map_err(|e| RegisterRepoError::ProfileLoad(MonjaProfileConfigError::Load(e)))?;
    config
        .save(&profile_config_path)
        .map_err(RegisterRepoError::ProfileWrite)?;

    Ok(RepoRegistration::AddedToExisting)
}

fn load_existing(
    profile_config_path: &Path,
) -> Result<Option<MonjaProfileConfig>, RegisterRepoError> {
    if !profile_config_path.exists() {
        return Ok(None);
    }

    let path = AbsolutePath::for_existing_path(profile_config_path)
        .map_err(|e| RegisterRepoError::ProfileLoad(MonjaProfileConfigError::Load(e)))?;

    MonjaProfileConfig::load(&path)
        .map(Some)
        .map_err(RegisterRepoError::ProfileLoad)
}

fn validate_against(
    config: &MonjaProfileConfig,
    repo_name: &RepoName,
) -> Result<(), RegisterRepoError> {
    // the two forms are mutually exclusive, so adding to `repos` would leave a profile that
    // fails to resolve at all -- worse than refusing up front.
    if config.repo_dir.is_some() {
        return Err(RegisterRepoError::LegacyRepoDir);
    }

    if config.repos.contains_key(repo_name) {
        return Err(RegisterRepoError::RepoAlreadyConfigured(repo_name.clone()));
    }

    Ok(())
}

// written by hand, rather than serialized, because the `[repos]` table has to come after every
// top-level key -- TOML would otherwise read those keys as belonging to the table -- and because
// a profile a user is about to edit reads better with the keys in a deliberate order.
fn create_profile(
    profile_config_path: &Path,
    configured_dir: &Path,
    repo_name: &RepoName,
    initial_set: Option<&SetName>,
) -> Result<(), RegisterRepoError> {
    let sets = initial_set
        .map(|s| format!("    '{}',\n", s))
        .unwrap_or_default();

    fs::write(
        profile_config_path,
        formatdoc! {"
            target-sets = [
            {sets}]

            default-repo = '{repo}'

            [repos]
            {repo} = '{dir}'
        ",
            sets = sets,
            repo = repo_name,
            dir = configured_dir.display(),
        },
    )
    .map_err(RegisterRepoError::ProfileCreate)
}
