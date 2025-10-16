// #![deny(exported_private_dependencies)]
#![deny(clippy::unwrap_used)]

use std::{
    collections::{HashMap, HashSet},
    ffi::{OsStr, OsString},
    io::Write,
    ops::Deref,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::LazyLock,
};

use clap::Args;
use relative_path::{PathExt, RelativePathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub(crate) mod local;
pub(crate) mod repo;
pub mod operation {
    pub mod clean;
    pub mod fix;
    pub mod init;
    pub mod pull;
    pub mod push;
    pub mod status;
}

pub use crate::{
    operation::clean::*, operation::fix::*, operation::init::*, operation::pull::*,
    operation::push::*, operation::status::*, repo::SetConfig, repo::SetConfigError, repo::SetName,
    repo::SetShortcutError,
};

//note that file index error is internal implementation detail
pub type LocalStateInitializationError = local::StateInitializationError;
pub type RepoStateInitializationError = repo::StateInitializationError;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MonjaProfileConfig {
    pub repo_dir: PathBuf,
    pub target_sets: Vec<SetName>,

    // TODO: probably remove
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_file_set: Option<SetName>,
}

#[derive(Error, Debug)]
pub enum MonjaProfileConfigError {
    #[error("Unable to deserialize monja-profile.toml.")]
    Deserialization(#[from] toml::de::Error),
    #[error("Unable to serialize monja-profile.toml.")]
    Serialization(#[from] toml::ser::Error),
    #[error("Unable to read from monja-profile.toml.")]
    Read(#[source] std::io::Error),
    #[error("Unable to write to monja-profile.toml.")]
    Write(#[source] std::io::Error),
}

impl MonjaProfileConfig {
    // we take a path to config file, not folder, since the profile could be one located in the repo, pointed to by local
    pub fn load(config_path: &AbsolutePath) -> Result<MonjaProfileConfig, MonjaProfileConfigError> {
        let config = std::fs::read(config_path).map_err(MonjaProfileConfigError::Read)?;

        Ok(toml::from_slice(&config)?)
    }

    pub fn save(&self, config_path: &AbsolutePath) -> Result<(), MonjaProfileConfigError> {
        std::fs::write(config_path, toml::to_string(&self)?)
            .map_err(MonjaProfileConfigError::Write)?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct MonjaProfile {
    pub local_root: AbsolutePath,
    pub repo_root: AbsolutePath,
    pub data_root: AbsolutePath,

    pub config: MonjaProfileConfig,
}

impl MonjaProfile {
    pub fn from_config(
        config: MonjaProfileConfig,
        local_root: AbsolutePath,
        data_root: AbsolutePath,
    ) -> Result<MonjaProfile, std::io::Error> {
        let repo_root = match config.repo_dir.is_relative() {
            true => AbsolutePath::for_existing_path(&local_root.join(&config.repo_dir))?,
            false => AbsolutePath::for_existing_path(&config.repo_dir)?,
        };

        Ok(MonjaProfile {
            local_root,
            repo_root,
            data_root,
            config,
        })
    }
}

// would ideally not depend on clap in this crate, but it's not worth the effort otherwise
// one alternative option is to expose a trait here, implemented main side
// another alternative is to use an object mapper like o2o
#[derive(Args)]
#[group(multiple = true)]
pub struct ExecutionOptions {
    #[arg(short, long = "verbose", action = clap::ArgAction::Count)]
    pub verbosity: u8,

    #[arg(long)]
    pub dry_run: bool,
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

impl Deref for AbsolutePath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl<T> AsRef<T> for AbsolutePath
where
    T: ?Sized,
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}

// the original types use private dependency RelativePathBuf, so we add these types to get around it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFilePath(PathBuf);

#[derive(Error, Debug)]
#[error("Unable to convert path to local file path.")]
pub struct LocalFilePathConversionError;

impl From<LocalFilePath> for PathBuf {
    fn from(value: LocalFilePath) -> Self {
        value.0
    }
}

impl LocalFilePath {
    pub fn from(profile: &MonjaProfile, path: &Path) -> Result<Self, LocalFilePathConversionError> {
        let path = RelativePathBuf::from_path(path).map_err(|_| LocalFilePathConversionError)?;
        let path = path.to_logical_path(&profile.local_root);
        if !path.starts_with(&profile.local_root) {
            return Err(LocalFilePathConversionError);
        }

        // not necessarily the same as the original, since we evaluated .. and .
        let path = path
            .relative_to(&profile.local_root)
            .map_err(|_| LocalFilePathConversionError)?;
        Ok(local::FilePath::new(path).into())
    }
}

impl From<local::FilePath> for LocalFilePath {
    fn from(value: local::FilePath) -> Self {
        LocalFilePath(value.to_path("".as_ref()))
    }
}

impl TryFrom<&LocalFilePath> for local::FilePath {
    type Error = relative_path::FromPathError;

    fn try_from(value: &LocalFilePath) -> Result<Self, Self::Error> {
        Ok(local::FilePath::new(
            relative_path::RelativePathBuf::from_path(value)?,
        ))
    }
}

impl Deref for LocalFilePath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> AsRef<T> for LocalFilePath
where
    T: ?Sized,
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}

impl PartialEq<Path> for LocalFilePath {
    fn eq(&self, other: &Path) -> bool {
        other == {
            let this: &Path = self;
            this
        }
    }
}

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

// not actually sure this is the best way, but it probably works
// and we can just test on windows if we ever support it
// test coverage also theoretically ensures we keep this list up to date
static MONJA_SPECIAL_FILES: LazyLock<HashSet<OsString>> = LazyLock::new(|| {
    HashSet::from([
        OsString::from(".monja-set.toml"),
        OsString::from(".monja-dir.toml"),
        OsString::from("monja-profile.toml"),
        OsString::from("monja-index.toml"),
        OsString::from("monja-index-prev.toml"),
        OsString::from(".monjaignore"),
    ])
});
pub fn is_monja_special_file(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|f: &OsStr| MONJA_SPECIAL_FILES.contains(f))
}

// keeping as io result because basically everything is io result
pub(crate) fn rsync(
    source: &Path,
    dest: &Path,
    files: impl Iterator<Item = PathBuf>,
    opts: &ExecutionOptions,
) -> std::io::Result<()> {
    // we use checksum mainly because, in integration tests, some files have same size and modified time
    // this could hypothetically happen in practice, so checksum is perhaps good.
    // note that file sizes still get compared before checksum, so most cases will still be fast.
    let mut args: Vec<&OsStr> = vec![
        "-a".as_ref(),
        "--files-from=-".as_ref(),
        "--checksum".as_ref(),
        "--mkpath".as_ref(),
    ];
    if opts.verbosity > 0 {
        args.push("-v".as_ref());
    }
    args.push(source.as_os_str());
    // append a /
    // works with mkpath to ensure the dir is properly created if needed
    let dest = dest.join("").into_os_string();
    args.push(&dest);

    let mut child = Command::new("rsync")
        .args(args)
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
    if opts.verbosity > 0 {
        println!("Finished rsync with status {}", status.status);
        // TODO: would be nice to return this instead?
        std::io::stderr().write_all(&status.stderr)?;
    }

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
