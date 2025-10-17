use std::{fs, path::PathBuf};

use indoc::{formatdoc, indoc};
use thiserror::Error;

use crate::{
    AbsolutePath, ExecutionOptions, MonjaProfile, MonjaProfileConfig, MonjaProfileConfigError,
};

#[derive(Error, Debug)]
pub enum InitError {
    #[error("monja has already been initialized.")]
    AlreadyInitialized,

    #[error("Failed to create monja-profile.")]
    Profile(#[source] std::io::Error),

    #[error("Failed to create set directory.")]
    Set(#[source] std::io::Error),

    #[error("Failed to create .monjaignore.")]
    IgnoreFile(#[source] std::io::Error),

    #[error("Failed to create README.md.")]
    Readme(#[source] std::io::Error),

    #[error("Failed to load newly created profile.")]
    ProfileLoad(#[from] MonjaProfileConfigError),
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
    pub repo_root: AbsolutePath,
    pub data_root: AbsolutePath,
    pub relative_repo_root: PathBuf,
    pub initial_set_name: String,
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

    fs::write(
        &spec.profile_config_path,
        formatdoc! {"
            repo-dir = '{}'

            target-sets = [
                '{}',
            ]
        ", spec.relative_repo_root.display(), &spec.initial_set_name },
    )
    .map_err(InitError::Profile)?;

    let set_path = spec.repo_root.join(spec.initial_set_name);
    fs::create_dir_all(&set_path).map_err(InitError::Set)?;

    fs::write(
        set_path.join(".monja-set.toml"),
        indoc! {"
            # Use a shortcut to reduce the amount of initial folder nesting!
            # shortcut = '.config'
        "},
    )
    .map_err(InitError::Profile)?;

    let ignorefile = spec.local_root.join(".monjaignore");
    if !ignorefile.exists() {
        fs::write(ignorefile, DEFAULT_IGNORE).map_err(InitError::IgnoreFile)?;
    }

    let readme = spec.repo_root.join("README.md");
    if !readme.exists() {
        fs::write(readme, README).map_err(InitError::Readme)?;
    }

    let profile = MonjaProfileConfig::load(
        &AbsolutePath::for_existing_path(&spec.profile_config_path)
            .expect("Just made the profile file."),
    )?;
    let profile = MonjaProfile::from_config(profile, spec.local_root, spec.data_root)
        .map_err(MonjaProfileConfigError::Read)?;

    Ok(InitSuccess {
        profile: Some(profile),
        profile_config_path: spec.profile_config_path,
    })
}

const DEFAULT_IGNORE: &str = indoc! {"
    # ignore files are used to keep stuff from getting to the repo from local, and to prevent local from being cleaned

    .*
    !.config/
    # it's recommended to put this in sets, since certain machines may have a different set
    !.monjaignore

    Desktop/
    Documents/
    Downloads/
    Music/
    Pictures/
    Public/
    Videos/
"};

const README: &str = indoc! {"
    ## monja
    This repo uses [monja](https://github.com/Timiz0r/monja) for managing dotfiles.

    To use the dotfiles in this repo:
    1. Install monja
    2. Clone this repo. The default path is `$XDG_DATA_HOME/monja/repo`, but anywhere works.
    3. Create a profile (see below)
    4. Run `monja pull`. Keep in mind this can overwrite existing files.

    ### Profiles
    A profile mainly specifies the set of directories found at the root of this repo (called sets).
    It lives in `$XDG_CONFIG_HOME/monja/monja-profile.toml`. Sample:

    ```toml
    # this can be an absolute path or a path relative to $HOME
    repo-dir = '.local/share/monja/repo'

    # these are layered on top of each other. if a file is in multiple sets, the later one wins.
    target-sets = [
        'foo',
        'bar',
        'baz',
    ]
    ```
"};
