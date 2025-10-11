// #![deny(exported_private_dependencies)]
#![deny(clippy::unwrap_used)]

use std::{
    collections::{HashMap, HashSet},
    ffi::{OsStr, OsString},
    io::Write,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::LazyLock,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub(crate) mod local;
pub(crate) mod repo;
pub mod operation {
    pub mod pull;
    pub mod push;
    pub mod status;
}

pub use crate::{
    operation::pull::*, operation::push::*, repo::SetConfig, repo::SetConfigError, repo::SetName,
    repo::SetShortcutError,
};

//note that file index error is internal implementation detail
pub type LocalStateInitializationError = local::StateInitializationError;
pub type RepoStateInitializationError = repo::StateInitializationError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MonjaProfileConfig {
    pub monja_dir: PathBuf,
    pub target_sets: Vec<SetName>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_file_set: Option<SetName>,
}

#[derive(Error, Debug)]
pub enum MonjaProfileConfigError {
    #[error("Unable to deserialize .monja-profile.toml.")]
    Deserialization(#[from] toml::de::Error),
    #[error("Unable to serialize .monja-profile.toml.")]
    Serialization(#[from] toml::ser::Error),
    #[error("Unable to save/load .monja-profile.toml.")]
    Io(#[from] std::io::Error),
}

impl MonjaProfileConfig {
    // we take a path to config file, not folder, since the profile could be one located in the repo, pointed to by local
    pub fn load(config_path: &AbsolutePath) -> Result<MonjaProfileConfig, MonjaProfileConfigError> {
        let config = std::fs::read(config_path.as_ref())?;

        Ok(toml::from_slice(&config)?)
    }

    pub fn save(&self, config_path: &AbsolutePath) -> Result<(), MonjaProfileConfigError> {
        Ok(std::fs::write(
            config_path.as_ref(),
            toml::to_string(&self)?,
        )?)
    }
}

#[derive(Debug)]
pub struct MonjaProfile {
    pub local_root: AbsolutePath,
    pub repo_root: AbsolutePath,

    pub config: MonjaProfileConfig,
}
impl MonjaProfile {
    pub fn from_config(
        config: MonjaProfileConfig,
        local_root: AbsolutePath,
    ) -> Result<MonjaProfile, std::io::Error> {
        Ok(MonjaProfile {
            local_root,
            repo_root: AbsolutePath::for_existing_path(&config.monja_dir)?,
            config,
        })
    }
}

#[derive(Debug)]
pub struct AbsolutePath {
    path: PathBuf,
}

impl AbsolutePath {
    pub fn for_existing_path(path: &Path) -> Result<AbsolutePath, std::io::Error> {
        std::fs::canonicalize(path).map(|path| AbsolutePath { path })
    }

    // could implement Into, but won't implement From because this is fallible and meant to use for_existing_path
    // could implement TryFrom, though, instead of naming it for_existing_path,
    // but we'd still want this because it doesn't copy
    pub fn into_path_buf(self) -> PathBuf {
        self.path
    }
}

impl AsRef<Path> for AbsolutePath {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

// the original types use private dependency RelativePathBuf, so we add these types to get around it
#[derive(Debug)]
pub struct LocalFilePath(PathBuf);
impl From<LocalFilePath> for PathBuf {
    fn from(value: LocalFilePath) -> Self {
        value.0
    }
}

impl From<local::FilePath> for LocalFilePath {
    fn from(value: local::FilePath) -> Self {
        LocalFilePath(value.into_relative_path_buf().to_path(""))
    }
}

impl AsRef<Path> for LocalFilePath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

#[derive(Debug)]
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

// not actually sure this is the best way, but it probably works
// and we can just test on windows if we ever support it
static MONJA_REPO_FILES: LazyLock<HashSet<OsString>> = LazyLock::new(|| {
    HashSet::from([
        OsString::from(".monja-set.toml"),
        OsString::from(".monja-dir.toml"),
    ])
});
static MONJA_LOCAL_FILES: LazyLock<HashSet<OsString>> = LazyLock::new(|| {
    HashSet::from([
        OsString::from(".monja-profile.toml"),
        OsString::from(".monja-index.toml"),
        OsString::from(".monjaignore.toml"),
    ])
});
pub fn is_monja_repo_file(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|f: &OsStr| MONJA_REPO_FILES.contains(f))
}
pub fn is_monja_local_file(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|f: &OsStr| MONJA_LOCAL_FILES.contains(f))
}

// keeping as io result because basically everything is io result
pub(crate) fn rsync(
    source: &Path,
    dest: &Path,
    files: impl Iterator<Item = PathBuf>,
) -> std::io::Result<()> {
    // we use checksum mainly because, in integration tests, some files have same size and modified time
    // this could hypothetically happen in practice, so checksum is perhaps good.
    // note that file sizes still get compared before checksum, so most cases will still be fast.
    let mut child = Command::new("rsync")
        .args([
            "-a".as_ref(),
            "--files-from=-".as_ref(),
            "--checksum".as_ref(),
            "--mkpath".as_ref(),
            source.as_os_str(),
            // works with mkpath to ensure the dir is properly created if needed
            dest.join("").as_os_str(),
        ])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    {
        let mut stdin = child.stdin.take().expect("Added above");
        for file in files {
            // avoiding the fallible conversion to string
            stdin.write_all(file.as_os_str().as_bytes())?;
            stdin.write_all(b"\n")?;
        }
        // dropping sends eof
    }

    let status = child.wait_with_output()?;
    println!("Finished rsync with status {}", status.status);
    // TODO: would be nice to return this instead?
    std::io::stderr().write_all(&status.stderr)?;

    match status.status.success() {
        true => Ok(()),
        false => Err(std::io::Error::other("Unsuccessful status code for rsync.")),
    }
}

// want to keep local/repo::File internal, so gonna bite the bullet on allocating another vector.
// this is mainly to avoid exporting RelativePath(Buf).
pub(crate) fn convert_set_file_result<Orig, Next>(
    // we use these sets to keep the ordering nice
    set_names: &[SetName],
    mut source: HashMap<repo::SetName, Vec<Orig>>,
) -> Vec<(repo::SetName, Vec<Next>)>
where
    Orig: Into<Next>,
{
    let mut result = Vec::with_capacity(source.len());

    result.extend(
        set_names
            .iter()
            .filter_map(|name| source.remove_entry(name))
            .map(|(name, set)| (name, set.into_iter().map(|p| p.into()).collect())),
    );

    result
}
