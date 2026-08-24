use std::{
    fs,
    path::{Path, PathBuf},
};

use indoc::indoc;
use thiserror::Error;

use crate::{
    AbsolutePath, ExecutionOptions, MonjaProfile, MonjaProfileConfig, MonjaProfileConfigError,
    PullError, RepoName, SetName, repo, set,
};

#[derive(Error, Debug)]
pub enum InitError {
    #[error(
        "monja has already been initialized. To add another repo to the profile, pass a repo name."
    )]
    AlreadyInitialized,

    #[error("Failed to add the repo to the monja-profile.")]
    Profile(#[from] repo::RegisterRepoError),

    #[error("Failed to create the repo directory '{0}'.")]
    RepoDirectory(PathBuf, #[source] std::io::Error),

    #[error("Failed to create set.")]
    Set(#[from] set::SetCreationError),

    #[error("Failed to create .monjaignore.")]
    IgnoreFile(#[source] std::io::Error),

    #[error("Failed to create README.md.")]
    Readme(#[source] std::io::Error),

    #[error("Failed to load newly created profile.")]
    ProfileLoad(#[from] MonjaProfileConfigError),

    #[error("Failed to perform initial pull.")]
    InitialPull(#[from] PullError),
}

// what a run of `init` actually did, since the same command both sets monja up from scratch and
// adds a repo to a profile that's already set up.
#[derive(Debug)]
pub enum InitOutcome {
    Initialized { initial_set: SetName },
    RepoAdded,
}

#[derive(Debug)]
pub struct InitSuccess {
    // only returns None on dryrun
    pub profile: Option<MonjaProfile>,
    pub profile_config_path: PathBuf,
    pub repo_name: RepoName,
    pub repo_root: PathBuf,
    pub outcome: InitOutcome,
}

pub struct InitSpec {
    // not AbsolutePath because it shouldn't exist
    pub profile_config_path: PathBuf,
    pub local_root: AbsolutePath,
    pub data_root: AbsolutePath,
    // None means the user didn't name a repo, which is what separates a plain `init` (an error
    // once a profile exists) from one that's explicitly asking for a new repo to be added.
    pub repo_name: Option<RepoName>,
    pub initial_set_name: String,
}

pub fn init(opts: &ExecutionOptions, spec: InitSpec) -> Result<InitSuccess, InitError> {
    match (spec.profile_config_path.exists(), spec.repo_name.as_ref()) {
        (false, _) => initialize(opts, spec),
        (true, None) => Err(InitError::AlreadyInitialized),
        (true, Some(_)) => add_repo(opts, spec),
    }
}

// adds an empty repo to an already-initialized profile. deliberately bare: no set, no README, no
// ignorefile, and no pull, since none of that is wanted a second time around -- and no git,
// which is `monja clone`'s job.
fn add_repo(opts: &ExecutionOptions, spec: InitSpec) -> Result<InitSuccess, InitError> {
    let repo_name = spec
        .repo_name
        .clone()
        .expect("Only called when a repo was named.");
    let repo_root = repo::repo_root_for(&spec.data_root, &repo_name);

    repo::validate_registration(&spec.profile_config_path, &repo_name)?;

    if opts.dry_run {
        return Ok(InitSuccess {
            profile: None,
            profile_config_path: spec.profile_config_path,
            repo_name,
            repo_root,
            outcome: InitOutcome::RepoAdded,
        });
    }

    let repo_root = create_repo_dir(&repo_root)?;
    repo::register_repo(
        &spec.profile_config_path,
        &spec.local_root,
        &repo_root,
        &repo_name,
        None,
    )?;

    let profile = load_profile(&spec.profile_config_path, spec.local_root, spec.data_root)?;

    Ok(InitSuccess {
        profile: Some(profile),
        profile_config_path: spec.profile_config_path,
        repo_name,
        repo_root: repo_root.into_path_buf(),
        outcome: InitOutcome::RepoAdded,
    })
}

fn initialize(opts: &ExecutionOptions, spec: InitSpec) -> Result<InitSuccess, InitError> {
    let repo_name = spec
        .repo_name
        .clone()
        .unwrap_or_else(RepoName::default_name);
    let repo_root = repo::repo_root_for(&spec.data_root, &repo_name);
    let initial_set = SetName(spec.initial_set_name.clone());

    if opts.dry_run {
        return Ok(InitSuccess {
            profile: None,
            profile_config_path: spec.profile_config_path,
            repo_name,
            repo_root,
            outcome: InitOutcome::Initialized { initial_set },
        });
    }

    let repo_root = create_repo_dir(&repo_root)?;

    repo::register_repo(
        &spec.profile_config_path,
        &spec.local_root,
        &repo_root,
        &repo_name,
        Some(&initial_set),
    )?;

    let profile = load_profile(&spec.profile_config_path, spec.local_root, spec.data_root)?;

    let set_path = set::create_empty_set(&profile, &repo_root, &initial_set)?;

    // goes before creating profile for move reasons
    let ignorefile = set_path.join(".monjaignore");
    fs::write(ignorefile, DEFAULT_IGNORE).map_err(InitError::IgnoreFile)?;

    let readme = repo_root.join("README.md");
    if !readme.exists() {
        fs::write(readme, README).map_err(InitError::Readme)?;
    }

    // any files placed in the set here (like .monjaignore) need to be pulled
    // we don't write directly to the local dir because we want them to be in the index
    crate::files::pull::pull(&profile, opts)?;

    Ok(InitSuccess {
        profile: Some(profile),
        profile_config_path: spec.profile_config_path,
        repo_name,
        repo_root: repo_root.into_path_buf(),
        outcome: InitOutcome::Initialized { initial_set },
    })
}

fn create_repo_dir(repo_root: &PathBuf) -> Result<AbsolutePath, InitError> {
    fs::create_dir_all(repo_root).map_err(|e| InitError::RepoDirectory(repo_root.clone(), e))?;

    AbsolutePath::for_existing_path(repo_root).map_err(|e| MonjaProfileConfigError::Load(e).into())
}

fn load_profile(
    profile_config_path: &Path,
    local_root: AbsolutePath,
    data_root: AbsolutePath,
) -> Result<MonjaProfile, InitError> {
    let config = MonjaProfileConfig::load(
        &AbsolutePath::for_existing_path(profile_config_path)
            .expect("The profile file is there by now."),
    )?;

    MonjaProfile::from_config(config, local_root, data_root)
        .map_err(|e| MonjaProfileConfigError::from(e).into())
}

const DEFAULT_IGNORE: &str = indoc! {"
    # ignore files are used to keep stuff from getting to the repo from local, and to prevent local from being cleaned

    # ignore files in root, but not dirs
    /*
    !/*/

    # no hidden files or dirs
    /.*
    # allow .config
    !/.config/
    # it's recommended to put this in sets, since certain machines may have a different set
    !**/.monjaignore

    /Desktop/
    /Documents/
    /Downloads/
    /Music/
    /Pictures/
    /Public/
    /Videos/
"};

const README: &str = indoc! {"
    ## monja
    This repo uses [monja](https://github.com/Timiz0r/monja) for managing dotfiles.

    To use the dotfiles in this repo:
    1. Install monja
    2. Run `monja clone --repo <name> <url of this repo>`.
       This clones it to `$XDG_DATA_HOME/monja/repos/<name>` and creates a profile if needed.
       (You can also clone it anywhere by hand and point a profile at it.)
    3. Choose the sets you want in the profile (see below)
    4. Run `monja file pull`. Keep in mind this can overwrite existing files.

    ### Profiles
    A profile mainly specifies the set of directories found at the root of this repo (called sets).
    It lives in `$XDG_CONFIG_HOME/monja/monja-profile.toml`. Sample:

    ```toml
    # these are layered on top of each other. if a file is in multiple sets, the later one wins.
    target-sets = [
        'foo',
        'bar',
        'baz',
    ]

    # which repo `monja newset` and `monja repodir` act on.
    # unnecessary when only one repo is configured.
    default-repo = 'default'

    # a profile can draw sets from any number of repos.
    # each path can be absolute or relative to $HOME.
    # a set name may only appear in one repo, otherwise it can't be resolved.
    [repos]
    default = '.local/share/monja/repos/default'
    ```
    "};
