use std::{
    fs,
    path::{Path, PathBuf},
};

use googletest::prelude::*;
use monja::{
    AbsolutePath, InitError, InitOutcome, InitSpec, InitSuccess, MonjaProfileConfig,
    RegisterRepoError, RepoName, SetName,
};

use crate::sim::{Simulator, set_names};

#[allow(dead_code)]
#[macro_use]
mod sim;

#[gtest]
fn files_placed_correctly() -> Result<()> {
    let sim = Simulator::create();
    fs::remove_file(sim.profile_path()).unwrap();

    // if this succeeds, the profile config definitely exists
    let result = init(&sim)?;
    let repo_root = init_repo_root(&sim, &RepoName::default_name());

    // the repo goes in a `repos/<name>` subdir, so a second repo can sit alongside the first
    expect_that!(
        repo_root,
        eq(&sim.data_root().join("repos").join(RepoName::DEFAULT))
    );
    expect_that!(
        result.profile.unwrap().repos[&RepoName::default_name()].to_path_buf(),
        eq(&repo_root)
    );

    expect_that!(repo_root.join("README.md").exists(), is_true());
    expect_that!(sim.local_root().join(".monjaignore").exists(), is_true());
    expect_that!(repo_root.join("initialset").exists(), is_true());
    expect_that!(
        repo_root.join("initialset/.monja-set.toml").exists(),
        is_true()
    );

    let dirs_in_repo: Vec<PathBuf> = repo_root
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

// naming a repo on an already-initialized profile means "add this repo", which is the one way
// `init` is allowed to do something to a profile that already exists.
#[gtest]
fn adds_repo_to_existing_profile() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["existing"]),
        ..old
    });

    let result = init_named(&sim, RepoName::from("second"))?;
    let repo_root = init_repo_root(&sim, &RepoName::from("second"));

    expect_that!(result.outcome, pat!(InitOutcome::RepoAdded));
    expect_that!(result.repo_root, eq(&repo_root));
    expect_that!(repo_root.is_dir(), is_true());

    let profile = result.profile.unwrap();
    expect_that!(
        profile.repo_names(),
        unordered_elements_are![eq(&RepoName::default_name()), eq(&RepoName::from("second"))]
    );
    // the repo is bare, and adding one says nothing about which sets are wanted
    expect_that!(
        profile.config.target_sets,
        elements_are![eq(&SetName("existing".into()))]
    );
    expect_that!(profile.config.default_repo, none());

    // no set, no README -- unlike a first-time init
    let entries: Vec<PathBuf> = repo_root
        .read_dir()?
        .filter_map(|r| r.map(|e| e.path()).ok())
        .collect();
    expect_that!(entries, is_empty());

    Ok(())
}

#[gtest]
fn errors_on_repo_name_already_in_profile() -> Result<()> {
    let sim = Simulator::create();

    let result = init_named(&sim, RepoName::default_name());

    expect_that!(
        result,
        err(pat!(InitError::Profile(pat!(
            RegisterRepoError::RepoAlreadyConfigured(anything())
        ))))
    );
    // nothing created for a repo we can't register
    expect_that!(
        init_repo_root(&sim, &RepoName::default_name()).exists(),
        is_false()
    );

    Ok(())
}

// the legacy single-repo form is mutually exclusive with `[repos]`, so adding to it would leave a
// profile that doesn't resolve at all.
#[gtest]
fn errors_on_legacy_repo_dir_profile() -> Result<()> {
    let sim = Simulator::create();
    let repo_dir = sim.repo_root().to_path_buf();
    sim.configure_profile(|old| MonjaProfileConfig {
        repo_dir: Some(repo_dir),
        repos: Default::default(),
        ..old
    });

    let result = init_named(&sim, RepoName::from("second"));

    expect_that!(
        result,
        err(pat!(InitError::Profile(pat!(
            RegisterRepoError::LegacyRepoDir
        ))))
    );

    Ok(())
}

#[gtest]
fn add_repo_dry_run() -> Result<()> {
    let mut sim = Simulator::create();
    sim.dryrun(true);

    let result = init_named(&sim, RepoName::from("second"))?;

    expect_that!(result.profile, none());
    expect_that!(
        init_repo_root(&sim, &RepoName::from("second")).exists(),
        is_false()
    );
    expect_that!(
        sim.profile()?.repo_names(),
        elements_are![eq(&RepoName::default_name())]
    );

    Ok(())
}

#[gtest]
fn profile_uses_the_requested_repo_name() -> Result<()> {
    let sim = Simulator::create();
    fs::remove_file(sim.profile_path()).unwrap();

    let result = init_named(&sim, RepoName::from("mycoolrepo"))?;

    // the repo is keyed under the requested name, and is the default, so that a fresh profile
    // needs no `--repo` for the commands that pick one
    let profile = result.profile.unwrap();
    expect_that!(
        profile.repo_names(),
        elements_are![eq(&RepoName::from("mycoolrepo"))]
    );
    expect_that!(
        profile.config.default_repo,
        some(eq(&RepoName::from("mycoolrepo")))
    );
    // the multi-repo form, not the legacy single-repo one
    expect_that!(profile.config.repo_dir, none());

    Ok(())
}

#[gtest]
fn dry_run() -> Result<()> {
    let mut sim = Simulator::create();
    sim.dryrun(true);
    fs::remove_file(sim.profile_path()).unwrap();

    let result = init(&sim)?;
    let repo_root = init_repo_root(&sim, &RepoName::default_name());
    expect_that!(result.profile, none());
    expect_that!(sim.profile_path().exists(), is_false());
    expect_that!(repo_root.join("initialset").exists(), is_false());
    expect_that!(
        repo_root.join("initialset/.monja-set.toml").exists(),
        is_false()
    );

    Ok(())
}

fn init(sim: &Simulator) -> std::result::Result<InitSuccess, InitError> {
    init_with_name(sim, None)
}

// where `init` puts a repo, which the sim's own `repo_root` isn't -- init derives it from the
// data root rather than being told, so that the layout is monja's decision and not the caller's.
fn init_repo_root(sim: &Simulator, repo_name: &RepoName) -> PathBuf {
    sim.data_root().join("repos").join(repo_name)
}

fn init_named(sim: &Simulator, repo_name: RepoName) -> std::result::Result<InitSuccess, InitError> {
    init_with_name(sim, Some(repo_name))
}

fn init_with_name(
    sim: &Simulator,
    repo_name: Option<RepoName>,
) -> std::result::Result<InitSuccess, InitError> {
    let spec = InitSpec {
        profile_config_path: sim.profile_path().to_path_buf(),
        local_root: AbsolutePath::for_existing_path(sim.local_root()).unwrap(),
        data_root: AbsolutePath::for_existing_path(sim.data_root()).unwrap(),
        initial_set_name: "initialset".into(),
        repo_name,
    };

    monja::init(sim.execution_options(), spec)
}
