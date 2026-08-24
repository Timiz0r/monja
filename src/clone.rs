use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use thiserror::Error;

use crate::{
    AbsolutePath, ExecutionOptions, MonjaProfile, MonjaProfileConfig, MonjaProfileConfigError,
    RepoName, repo,
};

#[derive(Error, Debug)]
pub enum CloneError {
    #[error("Unable to run git. Is it installed and on PATH?")]
    GitUnavailable(#[source] std::io::Error),

    #[error("The directory '{0}' already exists and isn't empty.")]
    DestinationNotEmpty(PathBuf),

    #[error("Unable to inspect the directory '{0}'.")]
    DestinationUnreadable(PathBuf, #[source] std::io::Error),

    #[error("Failed to create the repo directory '{0}'.")]
    RepoDirectory(PathBuf, #[source] std::io::Error),

    #[error("Failed to clone '{0}'.")]
    CloneFailed(String, #[source] std::io::Error),

    #[error("Unable to add the repo to the monja-profile.")]
    Profile(#[from] repo::RegisterRepoError),

    #[error("Failed to load the profile after cloning.")]
    ProfileLoad(#[from] MonjaProfileConfigError),
}

#[derive(Debug)]
pub struct CloneSuccess {
    // only returns None on dryrun
    pub profile: Option<MonjaProfile>,
    pub profile_config_path: PathBuf,
    pub repo_name: RepoName,
    pub repo_root: PathBuf,
    // whether the profile had to be created, as opposed to the repo joining an existing one
    pub profile_created: bool,
}

pub struct CloneSpec {
    // not AbsolutePath because it may not exist
    pub profile_config_path: PathBuf,
    pub local_root: AbsolutePath,
    pub data_root: AbsolutePath,
    pub repo_name: RepoName,
    pub url: String,
}

// clones a repo into the standard repo location and adds it to the profile, creating the profile
// if there isn't one yet.
//
// nothing else is created -- no set, no README, no ignorefile -- because a cloned repo already
// has whatever content it has. `target-sets` is likewise left alone: which of the repo's sets
// this machine wants is a decision only the user can make.
pub fn clone(opts: &ExecutionOptions, spec: CloneSpec) -> Result<CloneSuccess, CloneError> {
    // everything below this point either costs something or is visible to the user, so the
    // cheap "can we even do this" checks all happen first.
    ensure_git_available()?;

    // checked before cloning so that a name the profile can't accept doesn't leave a cloned repo
    // sitting on disk with nothing referring to it. it also goes before the destination check so
    // that re-cloning a repo already in the profile says so, rather than reporting the directory
    // that repo already occupies as merely being in the way.
    repo::validate_registration(&spec.profile_config_path, &spec.repo_name)?;

    let repo_root = repo::repo_root_for(&spec.data_root, &spec.repo_name);
    ensure_empty_destination(&repo_root)?;

    if opts.dry_run {
        let profile_created = !spec.profile_config_path.exists();
        return Ok(CloneSuccess {
            profile: None,
            profile_config_path: spec.profile_config_path,
            repo_name: spec.repo_name,
            repo_root,
            profile_created,
        });
    }

    // git creates the leaf itself, so we only make the parents -- which also means a failed
    // clone has nothing of ours to clean up beyond an empty `repos/`.
    if let Some(parent) = repo_root.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| CloneError::RepoDirectory(parent.to_path_buf(), e))?;
    }

    if let Err(e) = run_clone(&spec.url, &repo_root, opts) {
        // git cleans up after itself, but a partially-populated directory would make a retry
        // fail for the wrong reason, so make sure it's gone either way.
        let _ = fs::remove_dir_all(&repo_root);
        return Err(e);
    }

    let repo_root =
        AbsolutePath::for_existing_path(&repo_root).map_err(MonjaProfileConfigError::Load)?;

    let registration = repo::register_repo(
        &spec.profile_config_path,
        &spec.local_root,
        &repo_root,
        &spec.repo_name,
        None,
    )?;

    let config = MonjaProfileConfig::load(
        &AbsolutePath::for_existing_path(&spec.profile_config_path)
            .expect("The profile file is there by now."),
    )?;
    let profile = MonjaProfile::from_config(config, spec.local_root, spec.data_root)
        .map_err(MonjaProfileConfigError::from)?;

    Ok(CloneSuccess {
        profile: Some(profile),
        profile_config_path: spec.profile_config_path,
        repo_name: spec.repo_name,
        repo_root: repo_root.into_path_buf(),
        profile_created: registration == repo::RepoRegistration::CreatedProfile,
    })
}

// git is the one external tool monja only needs for this single command, so rather than
// abstracting around it, the two invocations live right here.

// deliberately separate from the clone itself so that a missing git is reported as a missing
// git, rather than as an inscrutable failure to clone.
fn ensure_git_available() -> Result<(), CloneError> {
    Command::new("git")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(CloneError::GitUnavailable)?;

    Ok(())
}

// stdio is inherited, rather than captured, so that progress output and any credential or
// host-key prompt reaches the user's terminal directly.
fn run_clone(url: &str, dest: &Path, opts: &ExecutionOptions) -> Result<(), CloneError> {
    let mut command = Command::new("git");
    command.arg("clone");
    if opts.verbosity == 0 {
        command.arg("--quiet");
    }
    command.arg(url).arg(dest);

    let status = command
        .status()
        .map_err(|e| CloneError::CloneFailed(url.to_string(), e))?;

    match status.success() {
        true => Ok(()),
        false => Err(CloneError::CloneFailed(
            url.to_string(),
            std::io::Error::other(format!("git clone exited with status {:?}", status.code())),
        )),
    }
}

fn ensure_empty_destination(repo_root: &Path) -> Result<(), CloneError> {
    if !repo_root.exists() {
        return Ok(());
    }

    let mut entries = fs::read_dir(repo_root)
        .map_err(|e| CloneError::DestinationUnreadable(repo_root.to_path_buf(), e))?;

    match entries.next() {
        None => Ok(()),
        Some(_) => Err(CloneError::DestinationNotEmpty(repo_root.to_path_buf())),
    }
}
