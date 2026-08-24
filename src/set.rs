use std::{
    collections::{BTreeMap, HashMap, btree_map},
    fmt::Display,
    fs,
    ops::Deref,
    path::{Path, PathBuf},
};

use indoc::indoc;
use relative_path::{RelativePath, RelativePathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{AbsolutePath, MonjaProfile, RepoName, repo::format_repo_names};

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct SetConfig {
    // used to be called root, but it was hard to disambiguate with other uses of the term
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<PathBuf>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<String>,
}

impl SetConfig {
    // takes the set's resolved root, rather than a profile, because a set name alone no longer
    // identifies a directory once a profile can have several repos.
    pub fn load(set_root: &Path, set_name: &SetName) -> Result<SetConfig, SetConfigError> {
        let config_path = set_root.join(".monja-set.toml");
        // is optional file
        let config = fs::read(config_path).unwrap_or_default();

        toml::from_slice(&config).map_err(|e| SetConfigError::Deserialization(set_name.clone(), e))
    }

    pub fn save(&self, set_root: &Path, set_name: &SetName) -> Result<(), SetConfigError> {
        fs::create_dir_all(set_root).map_err(|e| SetConfigError::Save(set_name.clone(), e))?;

        let config_path = set_root.join(".monja-set.toml");
        let config = toml::to_string(&self)
            .map_err(|e| SetConfigError::Serialization(set_name.clone(), e))?;

        fs::write(config_path, config).map_err(|e| SetConfigError::Save(set_name.clone(), e))
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SetName(pub String);
impl Display for SetName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for SetName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> AsRef<T> for SetName
where
    T: ?Sized,
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SetShortcut(RelativePathBuf);
impl SetShortcut {
    pub fn from_path(path: PathBuf) -> Result<Self, SetShortcutError> {
        let rel = RelativePathBuf::from_path(&path)
            .map_err(|e| SetShortcutError::NotRelative(path.clone(), e))?;

        let traversal_detection = rel.to_logical_path(".");
        if traversal_detection.as_path().as_os_str().is_empty() && !path.as_os_str().is_empty() {
            return Err(SetShortcutError::TraversalToParent(path));
        }

        Ok(SetShortcut(rel))
    }
}

impl<T> AsRef<T> for SetShortcut
where
    T: ?Sized,
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}

impl Deref for SetShortcut {
    type Target = RelativePath;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Error, Debug)]
pub enum SetConfigError {
    #[error("Unable to deserialize .monja-set.toml for set '{0}'.")]
    Deserialization(SetName, #[source] toml::de::Error),
    #[error("Unable to serialize .monja-set.toml for set '{0}'.")]
    Serialization(SetName, #[source] toml::ser::Error),
    #[error("Unable to save .monja-set.toml for set '{0}'.")]
    Save(SetName, #[source] std::io::Error),
}

#[derive(Error, Debug)]
pub enum SetShortcutError {
    #[error("Shortcut does not appear to be a relative path: {0}")]
    NotRelative(PathBuf, #[source] relative_path::FromPathError),
    #[error("Shortcut appears to be trying to traverse above the profile directory: {0}")]
    TraversalToParent(PathBuf),
}

#[derive(Error, Debug)]
pub enum SetCreationError {
    #[error("Failed to create sample .monja-set.toml.")]
    Config(SetName, #[source] std::io::Error),

    #[error("Failed to create set directory.")]
    SetCreation(SetName, #[source] std::io::Error),

    #[error("Set already exists.")]
    SetExists(SetName),
}
// raised when a command names a set that can't be resolved to exactly one directory.
// `Ambiguous` is deliberately distinct from `NotFound`: a set duplicated across repos is very
// much present, and reporting it as missing would send the user looking in the wrong place.
#[derive(Error, Debug)]
pub enum SetLookupError {
    #[error("Set '{0}' not found in any of the profile's repos.")]
    NotFound(SetName),

    #[error("Set '{name}' exists in multiple repos ({}), so it can't be resolved. Rename it in all but one of them.", format_repo_names(.repos))]
    Ambiguous { name: SetName, repos: Vec<RepoName> },
}

// the per-set state a mechanism (files, packages) loaded, plus the names it had to skip for
// being ambiguous. generic over the payload because files and packages load different things
// for a set (walked file contents vs. just a package list) but discover and resolve identically.
pub(crate) struct SetStates<T> {
    pub sets: HashMap<SetName, T>,
    // names found in more than one repo and not targeted by the profile. they're excluded from
    // `sets`, so they're kept here to tell an explicit `--set` reference apart from a typo.
    pub ambiguous_sets: BTreeMap<SetName, Vec<RepoName>>,
}

impl<T> SetStates<T> {
    // falls back to the skipped-for-ambiguity names so that an explicit reference to one says
    // so, rather than misreporting a very-much-present set as missing.
    pub(crate) fn get(&self, name: &SetName) -> Result<&T, SetLookupError> {
        if let Some(set) = self.sets.get(name) {
            return Ok(set);
        }

        match self.ambiguous_sets.get(name) {
            Some(repos) => Err(SetLookupError::Ambiguous {
                name: name.clone(),
                repos: repos.clone(),
            }),
            None => Err(SetLookupError::NotFound(name.clone())),
        }
    }
}

// discovers every set across the profile's repos and loads each resolvable one via `load`,
// leaving what that means entirely up to the caller.
//
// errors are collected rather than returned on the first failure, matching what the mechanisms
// did individually -- a repo with several problems should report all of them at once.
pub(crate) fn load_sets<T, E>(
    profile: &MonjaProfile,
    ambiguous: impl Fn(SetLookupError) -> E,
    load: impl Fn(&SetName, &SetLocation) -> Result<T, E>,
) -> Result<SetStates<T>, Vec<E>>
where
    E: From<DiscoverSetsError>,
{
    let index = discover_sets(profile)
        .map_err(|errs| errs.into_iter().map(Into::into).collect::<Vec<_>>())?;

    let mut sets = HashMap::new();
    let mut ambiguous_sets = BTreeMap::new();
    let mut errors = Vec::new();
    for (set_name, _) in index.iter() {
        let location = match index.resolve(set_name) {
            Ok(location) => location,
            // a name in several repos is only a problem if the profile actually uses it.
            // if it does, we fail; if it doesn't, we remember why it's missing and move on.
            Err(err) => {
                match (&err, profile.config.target_sets.contains(set_name)) {
                    (_, true) => errors.push(ambiguous(err)),
                    (SetLookupError::Ambiguous { repos, .. }, false) => {
                        ambiguous_sets.insert(set_name.clone(), repos.clone());
                    }
                    _ => (),
                }
                continue;
            }
        };

        match load(set_name, location) {
            Ok(set) => _ = sets.insert(set_name.clone(), set),
            Err(err) => errors.push(err),
        };
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(SetStates {
        sets,
        ambiguous_sets,
    })
}

// the shared discovery step every set-aware mechanism (file, package) builds on:
// enumerating which sets exist in the repos, without assuming anything about what a
// particular mechanism wants to load for each one (file contents vs. just config).
#[derive(Error, Debug)]
pub(crate) enum DiscoverSetsError {
    #[error("Unable to read the state of repo '{0}'.")]
    ReadSetDirs(RepoName, #[source] std::io::Error),
    #[error("Unable to convert dir name into set name: {0:?}")]
    NonUtf8Path(std::ffi::OsString),
}

#[derive(Debug, Clone)]
pub(crate) struct SetLocation {
    pub repo: RepoName,
    pub root: PathBuf,
}

// every set name found across every configured repo, along with each place it was found.
// duplicates aren't rejected here on purpose: a collision between two repos only matters if the
// profile actually uses the name, and failing discovery outright would block unrelated commands.
struct SetIndex {
    by_name: BTreeMap<SetName, Vec<SetLocation>>,
}

impl SetIndex {
    fn iter(&self) -> btree_map::Iter<'_, SetName, Vec<SetLocation>> {
        self.by_name.iter()
    }

    fn resolve(&self, name: &SetName) -> Result<&SetLocation, SetLookupError> {
        match self.by_name.get(name).map(Vec::as_slice) {
            None | Some([]) => Err(SetLookupError::NotFound(name.clone())),
            Some([location]) => Ok(location),
            Some(locations) => Err(SetLookupError::Ambiguous {
                name: name.clone(),
                repos: locations.iter().map(|l| l.repo.clone()).collect(),
            }),
        }
    }
}

fn discover_sets(profile: &MonjaProfile) -> Result<SetIndex, Vec<DiscoverSetsError>> {
    let mut by_name: BTreeMap<SetName, Vec<SetLocation>> = BTreeMap::new();
    let mut errors = Vec::new();

    for (repo_name, repo_root) in profile.repos.iter() {
        // while we'll prefer to collect errors into a vector, there's no point in continuing
        // with a repo we can't even read the root of.
        let read_dir = match fs::read_dir(repo_root) {
            Ok(read_dir) => read_dir,
            Err(e) => {
                errors.push(DiscoverSetsError::ReadSetDirs(repo_name.clone(), e));
                continue;
            }
        };

        for result in read_dir {
            match result {
                Err(err) => errors.push(DiscoverSetsError::ReadSetDirs(repo_name.clone(), err)),
                Ok(e) if e.path().is_dir() => {
                    match e.file_name().into_string() {
                        Ok(str) => by_name.entry(SetName(str)).or_default().push(SetLocation {
                            repo: repo_name.clone(),
                            root: e.path(),
                        }),
                        Err(initial) => errors.push(DiscoverSetsError::NonUtf8Path(initial)),
                    };
                }
                _ => (), // non-dirs
            };
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(SetIndex { by_name })
}

pub(crate) fn create_empty_set(
    profile: &MonjaProfile,
    repo_root: &AbsolutePath,
    name: &SetName,
) -> Result<AbsolutePath, SetCreationError> {
    // checking every repo, not just the target one, because creating a name that already exists
    // elsewhere would manufacture exactly the ambiguity that set resolution refuses to guess at.
    if profile.repo_roots().any(|root| root.join(name).exists()) {
        return Err(SetCreationError::SetExists(name.clone()));
    }

    let set_path = repo_root.join(name);
    fs::create_dir_all(&set_path).map_err(|e| SetCreationError::SetCreation(name.clone(), e))?;
    fs::write(
        set_path.join(".monja-set.toml"),
        indoc! {"
            # Use a shortcut to reduce the amount of initial folder nesting!
            # shortcut = '.config'
        "},
    )
    .map_err(|e| SetCreationError::Config(name.clone(), e))?;

    Ok(AbsolutePath::for_existing_path(&set_path).expect("Just created it."))
}
