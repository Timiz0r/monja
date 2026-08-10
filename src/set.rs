use std::{fmt::Display, fs, ops::Deref, path::PathBuf};

use indoc::indoc;
use relative_path::{RelativePath, RelativePathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{AbsolutePath, MonjaProfile};

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
    pub fn load(
        profile: &crate::MonjaProfile,
        set_name: &SetName,
    ) -> Result<SetConfig, SetConfigError> {
        let config_path = profile.repo_root.join(set_name).join(".monja-set.toml");
        // is optional file
        let config = fs::read(config_path).unwrap_or_default();

        toml::from_slice(&config).map_err(|e| SetConfigError::Deserialization(set_name.clone(), e))
    }

    pub fn save(&self, profile: &MonjaProfile, set_name: &SetName) -> Result<(), SetConfigError> {
        let set_dir = profile.repo_root.join(set_name);
        fs::create_dir_all(&set_dir).map_err(|e| SetConfigError::Save(set_name.clone(), e))?;

        let config_path = set_dir.join(".monja-set.toml");
        let config = toml::to_string(&self)
            .map_err(|e| SetConfigError::Serialization(set_name.clone(), e))?;

        fs::write(config_path, config).map_err(|e| SetConfigError::Save(set_name.clone(), e))
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize)]
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

// the shared discovery step every set-aware mechanism (file, package) builds on:
// enumerating which sets exist in the repo, without assuming anything about what a
// particular mechanism wants to load for each one (file contents vs. just config).
#[derive(Error, Debug)]
pub(crate) enum DiscoverSetsError {
    #[error("Unable to read the state of the repo.")]
    ReadSetDirs(#[source] std::io::Error),
    #[error("Unable to convert dir name into set name: {0:?}")]
    NonUtf8Path(std::ffi::OsString),
}

pub(crate) fn discover_sets(
    profile: &MonjaProfile,
) -> Result<Vec<(SetName, PathBuf)>, Vec<DiscoverSetsError>> {
    // while we'll prefer to collect errors into a vector, there's no point in continuing if we can't read this dir.
    let read_dir =
        fs::read_dir(&profile.repo_root).map_err(|e| vec![DiscoverSetsError::ReadSetDirs(e)])?;

    let mut set_info = Vec::new();
    let mut errors = Vec::new();

    for result in read_dir {
        match result {
            Err(err) => errors.push(DiscoverSetsError::ReadSetDirs(err)),
            Ok(e) if e.path().is_dir() => {
                match e.file_name().into_string() {
                    Ok(str) => set_info.push((SetName(str), e.path())),
                    Err(initial) => errors.push(DiscoverSetsError::NonUtf8Path(initial)),
                };
            }
            _ => (), // non-dirs
        };
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(set_info)
}

pub(crate) fn create_empty_set(
    profile: &MonjaProfile,
    name: &SetName,
) -> Result<AbsolutePath, SetCreationError> {
    let set_path = profile.repo_root.join(name);
    if set_path.exists() {
        return Err(SetCreationError::SetExists(name.clone()));
    }

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
