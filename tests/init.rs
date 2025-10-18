use std::{
    fs,
    path::{Path, PathBuf},
};

use googletest::prelude::*;
use monja::{AbsolutePath, InitError, InitSpec, InitSuccess};
use relative_path::PathExt;

use crate::sim::Simulator;

#[allow(dead_code)]
#[macro_use]
mod sim;

#[gtest]
fn files_placed_correctly() -> Result<()> {
    let sim = Simulator::create();
    fs::remove_file(sim.profile_path()).unwrap();

    // if this succeeds, the profile config definitely exists
    let _result = init(&sim)?;

    expect_that!(sim.repo_root().join("README.md").exists(), is_true());
    expect_that!(sim.local_root().join(".monjaignore").exists(), is_true());
    expect_that!(sim.repo_root().join("initialset").exists(), is_true());
    expect_that!(
        sim.repo_root().join("initialset/.monja-set.toml").exists(),
        is_true()
    );

    let dirs_in_repo: Vec<PathBuf> = sim
        .repo_root()
        .read_dir()?
        .filter_map(|r| r.map(|e| e.path()).ok())
        .filter(|p| p.is_dir())
        .collect();
    // contains one dir. we don't really need to know the name
    expect_that!(dirs_in_repo, { anything() });

    Ok(())
}

#[gtest]
fn ignorefile_exceptions_correct() -> Result<()> {
    let sim = Simulator::create();
    fs::remove_file(sim.profile_path()).unwrap();

    let _result = init(&sim)?;

    fs_operation! { LocalManipulation, sim,
        dir ".config"
            file "notinrepo" "notinrepo"
        end
        dir ".local/share"
            file "notinrepo" "notinrepo"
        end
        dir ".foobar"
            file "notinrepo" "notinrepo"
        end
    };

    let status = monja::local_status(&sim.profile()?, sim.cwd())?;
    expect_that!(status.untracked_files, {
        eq(Path::new(".config/notinrepo"))
    });

    Ok(())
}

#[gtest]
fn errors_on_existing_profile() -> Result<()> {
    let sim = Simulator::create();
    // note we're not removing it
    // fs::remove_file(sim.profile_path()).unwrap();

    let result = init(&sim);

    expect_that!(result, err(pat!(InitError::AlreadyInitialized)));

    Ok(())
}

#[gtest]
fn dry_run() -> Result<()> {
    let mut sim = Simulator::create();
    sim.dryrun(true);
    fs::remove_file(sim.profile_path()).unwrap();

    let result = init(&sim)?;
    expect_that!(result.profile, none());
    expect_that!(sim.profile_path().exists(), is_false());
    expect_that!(sim.repo_root().join("initialset").exists(), is_false());
    expect_that!(
        sim.repo_root().join("initialset/.monja-set.toml").exists(),
        is_false()
    );

    Ok(())
}

fn init(sim: &Simulator) -> std::result::Result<InitSuccess, InitError> {
    let spec = InitSpec {
        profile_config_path: sim.profile_path().to_path_buf(),
        local_root: AbsolutePath::for_existing_path(sim.local_root()).unwrap(),
        repo_root: AbsolutePath::for_existing_path(sim.repo_root()).unwrap(),
        data_root: AbsolutePath::for_existing_path(sim.data_root()).unwrap(),
        relative_repo_root: sim
            .repo_root()
            .relative_to(sim.local_root())
            .unwrap()
            .to_path(""),
        initial_set_name: "initialset".into(),
    };

    monja::init(sim.execution_options(), spec)
}
