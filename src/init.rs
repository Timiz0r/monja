use std::{fs, path::PathBuf};

use indoc::{formatdoc, indoc};
use thiserror::Error;

use crate::{
    AbsolutePath, ExecutionOptions, MonjaProfile, MonjaProfileConfig, MonjaProfileConfigError,
    PullError, RepoName, SetName, set,
};

#[derive(Error, Debug)]
pub enum InitError {
    #[error("monja has already been initialized.")]
    AlreadyInitialized,

    #[error("Failed to create monja-profile.")]
    Profile(#[source] std::io::Error),

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

#[derive(Debug)]
pub struct InitSuccess {
    // only returns None on dryrun
    pub profile: Option<MonjaProfile>,
    pub profile_config_path: PathBuf,
}

pub struct InitSpec {
    // not AbsolutePath because it shouldn't exist
    pub profile_config_path: PathBuf,
    pub local_root: AbsolutePath,
    pub data_root: AbsolutePath,
    pub repo_name: RepoName,
    pub initial_set_name: String,
}

// where a repo created by `init` lives, relative to the data root. a `repos/<name>` directory
// rather than a single `repo` one, so that adding a second repo later doesn't need a different
// layout than the first.
fn repo_root_for(data_root: &AbsolutePath, repo_name: &RepoName) -> PathBuf {
    data_root.join("repos").join(repo_name)
}

pub fn init(opts: &ExecutionOptions, spec: InitSpec) -> Result<InitSuccess, InitError> {
    if spec.profile_config_path.exists() {
        return Err(InitError::AlreadyInitialized);
    }

    if opts.dry_run {
        return Ok(InitSuccess {
            profile: None,
            profile_config_path: spec.profile_config_path,
        });
    }

    let repo_root = repo_root_for(&spec.data_root, &spec.repo_name);
    fs::create_dir_all(&repo_root).map_err(|e| InitError::RepoDirectory(repo_root.clone(), e))?;
    let repo_root =
        AbsolutePath::for_existing_path(&repo_root).map_err(MonjaProfileConfigError::Load)?;

    // a path under the local root is recorded relative to it, so a profile stays portable
    // across machines whose home directories differ.
    let configured_dir = repo_root
        .strip_prefix(&spec.local_root)
        .unwrap_or(&repo_root)
        .to_path_buf();

    // the `[repos]` table has to come after every top-level key, or TOML would read those keys
    // as belonging to the table.
    fs::write(
        &spec.profile_config_path,
        formatdoc! {"
            target-sets = [
                '{set}',
            ]

            default-repo = '{repo}'

            [repos]
            {repo} = '{dir}'
        ",
            set = &spec.initial_set_name,
            repo = &spec.repo_name,
            dir = configured_dir.display(),
        },
    )
    .map_err(InitError::Profile)?;

    let profile = MonjaProfileConfig::load(
        &AbsolutePath::for_existing_path(&spec.profile_config_path)
            .expect("Just made the profile file."),
    )?;
    let profile = MonjaProfile::from_config(profile, spec.local_root, spec.data_root)
        .map_err(MonjaProfileConfigError::from)?;

    let set_path = set::create_empty_set(
        &profile,
        &repo_root,
        &SetName(spec.initial_set_name.clone()),
    )?;

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
    })
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
    2. Clone this repo. The default path is `$XDG_DATA_HOME/monja/repos/<name>`, but anywhere works.
    3. Create a profile (see below)
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
