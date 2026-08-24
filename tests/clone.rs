use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use googletest::prelude::*;
use monja::{
    AbsolutePath, CloneError, CloneSpec, CloneSuccess, MonjaProfileConfig, RegisterRepoError,
    RepoName, SetName,
};
use tempfile::TempDir;

use crate::sim::{Simulator, set_names};

#[allow(dead_code)]
#[macro_use]
mod sim;

#[gtest]
fn creates_profile_when_missing() -> Result<()> {
    let sim = Simulator::create();
    fs::remove_file(sim.profile_path()).unwrap();
    let source = SourceRepo::create();

    let result = clone(&sim, &source, RepoName::from("cloned"))?;
    let repo_root = clone_repo_root(&sim, &RepoName::from("cloned"));

    // the standard location, same as the one `init` uses
    expect_that!(result.repo_root, eq(&repo_root));
    expect_that!(result.profile_created, is_true());
    expect_that!(
        fs::read_to_string(repo_root.join("clonedset/fromclone"))?,
        eq("cloned")
    );

    let profile = result.profile.unwrap();
    expect_that!(
        profile.repo_names(),
        elements_are![eq(&RepoName::from("cloned"))]
    );
    expect_that!(
        profile.config.default_repo,
        some(eq(&RepoName::from("cloned")))
    );
    // which of the repo's sets this machine wants is the user's call, not ours
    expect_that!(profile.config.target_sets, is_empty());
    // no scaffolding of any kind -- the repo brings its own content
    expect_that!(sim.local_root().join(".monjaignore").exists(), is_false());

    Ok(())
}

#[gtest]
fn adds_repo_to_existing_profile() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["existing"]),
        ..old
    });
    let source = SourceRepo::create();

    let result = clone(&sim, &source, RepoName::from("cloned"))?;

    expect_that!(result.profile_created, is_false());

    let profile = result.profile.unwrap();
    expect_that!(
        profile.repo_names(),
        unordered_elements_are![eq(&RepoName::default_name()), eq(&RepoName::from("cloned"))]
    );
    // an existing profile's targeting and default are the user's, and cloning says nothing
    // about either
    expect_that!(
        profile.config.target_sets,
        elements_are![eq(&SetName("existing".into()))]
    );
    expect_that!(profile.config.default_repo, none());

    Ok(())
}

// the point of cloning into the standard location and registering it: the sets in the cloned
// repo behave like any other set once the profile targets them.
#[gtest]
fn cloned_sets_are_usable() -> Result<()> {
    let sim = Simulator::create();
    fs::remove_file(sim.profile_path()).unwrap();
    let source = SourceRepo::create();

    clone(&sim, &source, RepoName::from("cloned"))?;

    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["clonedset"]),
        ..old
    });
    monja::pull(&sim.profile()?, sim.execution_options())?;

    expect_that!(
        fs::read_to_string(sim.local_root().join("fromclone"))?,
        eq("cloned")
    );

    Ok(())
}

#[gtest]
fn errors_on_clone_failure() -> Result<()> {
    let sim = Simulator::create();
    fs::remove_file(sim.profile_path()).unwrap();
    let source = SourceRepo::create();
    let missing = source.dir.path().join("not-a-repo");

    let result = clone_from(
        &sim,
        &missing.display().to_string(),
        RepoName::from("cloned"),
    );

    expect_that!(
        result,
        err(pat!(CloneError::CloneFailed(anything(), anything())))
    );
    // a failed clone leaves nothing behind to confuse a retry
    expect_that!(
        clone_repo_root(&sim, &RepoName::from("cloned")).exists(),
        is_false()
    );
    expect_that!(sim.profile_path().exists(), is_false());

    Ok(())
}

// a name the profile can't accept is rejected before git runs, so we never clone a repo that
// nothing would end up referring to.
#[gtest]
fn errors_on_repo_name_already_in_profile() -> Result<()> {
    let sim = Simulator::create();
    let source = SourceRepo::create();

    // the directory the repo would go in is also occupied, to confirm the name collision -- the
    // actual reason -- is what gets reported, rather than the directory merely being in the way
    let repo_root = clone_repo_root(&sim, &RepoName::default_name());
    fs::create_dir_all(&repo_root)?;
    fs::write(repo_root.join("occupied"), "occupied")?;

    let result = clone(&sim, &source, RepoName::default_name());

    expect_that!(
        result,
        err(pat!(CloneError::Profile(pat!(
            RegisterRepoError::RepoAlreadyConfigured(anything())
        ))))
    );

    Ok(())
}

#[gtest]
fn errors_on_legacy_repo_dir_profile() -> Result<()> {
    let sim = Simulator::create();
    let repo_dir = sim.repo_root().to_path_buf();
    sim.configure_profile(|old| MonjaProfileConfig {
        repo_dir: Some(repo_dir),
        repos: Default::default(),
        ..old
    });
    let source = SourceRepo::create();

    let result = clone(&sim, &source, RepoName::from("cloned"));

    expect_that!(
        result,
        err(pat!(CloneError::Profile(pat!(
            RegisterRepoError::LegacyRepoDir
        ))))
    );
    expect_that!(
        clone_repo_root(&sim, &RepoName::from("cloned")).exists(),
        is_false()
    );

    Ok(())
}

#[gtest]
fn errors_on_nonempty_destination() -> Result<()> {
    let sim = Simulator::create();
    let source = SourceRepo::create();

    let repo_root = clone_repo_root(&sim, &RepoName::from("cloned"));
    fs::create_dir_all(&repo_root)?;
    fs::write(repo_root.join("inthewaydontclobberme"), "keep me")?;

    let result = clone(&sim, &source, RepoName::from("cloned"));

    expect_that!(
        result,
        err(pat!(CloneError::DestinationNotEmpty(anything())))
    );
    expect_that!(
        fs::read_to_string(repo_root.join("inthewaydontclobberme"))?,
        eq("keep me")
    );

    Ok(())
}

#[gtest]
fn dry_run() -> Result<()> {
    let mut sim = Simulator::create();
    sim.dryrun(true);
    fs::remove_file(sim.profile_path()).unwrap();
    let source = SourceRepo::create();

    let result = clone(&sim, &source, RepoName::from("cloned"))?;

    expect_that!(result.profile, none());
    expect_that!(
        clone_repo_root(&sim, &RepoName::from("cloned")).exists(),
        is_false()
    );
    expect_that!(sim.profile_path().exists(), is_false());

    Ok(())
}

// where `clone` puts a repo. derived from the data root by monja rather than being told, exactly
// as `init` does, so tests can assert the layout instead of dictating it.
fn clone_repo_root(sim: &Simulator, repo_name: &RepoName) -> PathBuf {
    sim.data_root().join("repos").join(repo_name)
}

fn clone(
    sim: &Simulator,
    source: &SourceRepo,
    repo_name: RepoName,
) -> std::result::Result<CloneSuccess, CloneError> {
    clone_from(sim, &source.url(), repo_name)
}

fn clone_from(
    sim: &Simulator,
    url: &str,
    repo_name: RepoName,
) -> std::result::Result<CloneSuccess, CloneError> {
    let spec = CloneSpec {
        profile_config_path: sim.profile_path().to_path_buf(),
        local_root: AbsolutePath::for_existing_path(sim.local_root()).unwrap(),
        data_root: AbsolutePath::for_existing_path(sim.data_root()).unwrap(),
        repo_name,
        url: url.to_string(),
    };

    monja::clone(sim.execution_options(), spec)
}

// a real git repo to clone from, so the tests exercise the actual git invocation rather than a
// stand-in for it. cloned from a local path, which git handles the same way as any other URL.
struct SourceRepo {
    dir: TempDir,
}

impl SourceRepo {
    fn create() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("MonjaSource")
            .tempdir()
            .unwrap();

        git(dir.path(), &["init", "--quiet"]);
        fs::create_dir_all(dir.path().join("clonedset")).unwrap();
        fs::write(dir.path().join("clonedset/fromclone"), "cloned").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "--quiet", "-m", "initial"]);

        SourceRepo { dir }
    }

    fn url(&self) -> String {
        self.dir.path().display().to_string()
    }
}

// every setting the commands need is passed with `-c`, so the tests don't depend on -- or get
// tripped up by -- whatever git config the machine running them happens to have.
fn git(cwd: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=monja test",
            "-c",
            "user.email=monja@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(cwd)
        .status()
        .unwrap();

    assert!(
        status.success(),
        "git {:?} failed in {}",
        args,
        cwd.display()
    );
}
