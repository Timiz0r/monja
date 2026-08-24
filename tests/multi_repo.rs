use std::path::Path;

use googletest::prelude::*;

use crate::sim::{Simulator, set_names};
use monja::{
    AddError, MonjaProfile, MonjaProfileConfig, MonjaProfileConfigError, NewSetError, ProfileError,
    PullError, PutError, RepoName, RepoSelectionError, RepoStateInitializationError,
    SetLookupError, SetName,
};

#[allow(dead_code)]
#[macro_use]
mod sim;

// a profile drawing sets from two repos should behave exactly as if every set had been in one
// repo -- including `target-sets` ordering deciding which set's copy of a file wins.
#[gtest]
fn pull_merges_sets_across_repos() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["fromdefault", "fromother"]),
        ..old
    });

    fs_operation! { RepoSetManipulation, sim, "default", "fromdefault",
        file "only-in-default" "a"
        file "shared" "loses"
    };
    fs_operation! { RepoSetManipulation, sim, "other", "fromother",
        file "only-in-other" "b"
        file "shared" "wins"
    };

    let result = monja::pull(&sim.profile()?, sim.execution_options())?;
    expect_that!(
        result
            .files_pulled
            .into_iter()
            .map(|(s, _)| s)
            .collect::<Vec<_>>(),
        elements_are![
            eq(&SetName("fromdefault".into())),
            eq(&SetName("fromother".into()))
        ]
    );

    fs_operation! { LocalValidation, sim,
        file "only-in-default" "a"
        file "only-in-other" "b"
        // later set wins, exactly as it would within a single repo
        file "shared" "wins"
    };

    Ok(())
}

// reversing target-sets should reverse the winner, confirming that precedence really does come
// from target-sets and not from which repo a set happens to live in.
#[gtest]
fn target_set_order_decides_winner_not_repo_order() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["fromother", "fromdefault"]),
        ..old
    });

    fs_operation! { RepoSetManipulation, sim, "default", "fromdefault",
        file "shared" "default wins"
    };
    fs_operation! { RepoSetManipulation, sim, "other", "fromother",
        file "shared" "other loses"
    };

    monja::pull(&sim.profile()?, sim.execution_options())?;

    fs_operation! { LocalValidation, sim,
        file "shared" "default wins"
    };

    Ok(())
}

#[gtest]
fn duplicate_set_name_in_target_sets_errors() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["dupe"]),
        ..old
    });

    fs_operation! { RepoSetManipulation, sim, "default", "dupe",
        file "a" "a"
    };
    fs_operation! { RepoSetManipulation, sim, "other", "dupe",
        file "b" "b"
    };

    let result = monja::pull(&sim.profile()?, sim.execution_options());
    let specific_error = contains(pat!(RepoStateInitializationError::AmbiguousSet(pat!(
        SetLookupError::Ambiguous { .. }
    ))));
    expect_that!(
        result,
        err(pat!(PullError::RepoStateInitialization(specific_error)))
    );

    Ok(())
}

// a collision between two repos that the profile never uses shouldn't stop unrelated work --
// otherwise adding a second repo could break commands that have nothing to do with it.
#[gtest]
fn untargeted_duplicate_set_name_is_ignored() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["used"]),
        ..old
    });

    fs_operation! { RepoSetManipulation, sim, "default", "used",
        file "wanted" "wanted"
    };
    fs_operation! { RepoSetManipulation, sim, "default", "dupe",
        file "a" "a"
    };
    fs_operation! { RepoSetManipulation, sim, "other", "dupe",
        file "b" "b"
    };

    monja::pull(&sim.profile()?, sim.execution_options())?;

    fs_operation! { LocalValidation, sim,
        file "wanted" "wanted"
    };

    Ok(())
}

// ...but naming that duplicate explicitly has to say it's ambiguous, not that it's missing:
// the set very much exists, and "not found" would send the user looking in the wrong place.
#[gtest]
fn explicit_reference_to_untargeted_duplicate_errors_as_ambiguous() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: Vec::new(),
        ..old
    });

    fs_operation! { RepoSetManipulation, sim, "default", "dupe",
        file "a" "a"
    };
    fs_operation! { RepoSetManipulation, sim, "other", "dupe",
        file "b" "b"
    };
    fs_operation! { LocalManipulation, sim,
        file "newfile" "newfile"
    };

    let result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("newfile")],
        SetName("dupe".into()),
    );
    expect_that!(
        result,
        err(pat!(PutError::AmbiguousSet(pat!(
            SetLookupError::Ambiguous { .. }
        ))))
    );

    let result = monja::add(
        &sim.profile()?,
        sim.execution_options(),
        SetName("dupe".into()),
        vec!["neovim".into()],
    );
    expect_that!(
        result,
        err(pat!(AddError::AmbiguousSet(pat!(
            SetLookupError::Ambiguous { .. }
        ))))
    );

    Ok(())
}

// a genuinely absent set still has to report as missing, so that the ambiguity handling above
// hasn't simply swallowed the not-found case.
#[gtest]
fn missing_set_still_reports_as_not_found() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");

    fs_operation! { LocalManipulation, sim,
        file "newfile" "newfile"
    };

    let result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("newfile")],
        SetName("nosuchset".into()),
    );
    expect_that!(
        result,
        err(pat!(PutError::SetNotFound(&SetName("nosuchset".into()))))
    );

    Ok(())
}

#[gtest]
fn put_targets_the_set_in_its_own_repo() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["inother"]),
        ..old
    });

    fs_operation! { RepoSetManipulation, sim, "other", "inother",
        file "existing" "existing"
    };
    fs_operation! { LocalManipulation, sim,
        file "newfile" "newfile"
    };

    let result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("newfile")],
        SetName("inother".into()),
    )?;
    expect_that!(result.repo, eq(&RepoName::from("other")));

    fs_operation! { RepoSetValidation, sim, "other", "inother",
        file "existing" "existing"
        file "newfile" "newfile"
    };

    Ok(())
}

#[gtest]
fn transfer_moves_files_between_repos() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["source", "dest"]),
        ..old
    });

    fs_operation! { RepoSetManipulation, sim, "default", "source",
        file "moved" "moved"
        file "stays" "stays"
    };
    fs_operation! { RepoSetManipulation, sim, "other", "dest",
        file "already" "already"
    };

    monja::pull(&sim.profile()?, sim.execution_options())?;

    monja::transfer(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("moved")],
        SetName("source".into()),
        SetName("dest".into()),
    )?;

    fs_operation! { RepoSetValidation, sim, "default", "source",
        file "stays" "stays"
    };
    fs_operation! { RepoSetValidation, sim, "other", "dest",
        file "already" "already"
        file "moved" "moved"
    };

    Ok(())
}

#[gtest]
fn packages_merge_across_repos() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });

    fs_operation! { RepoSetManipulation, sim, "default", "set1",
        file "placeholder" "placeholder"
    };
    fs_operation! { RepoSetManipulation, sim, "other", "set2",
        file "placeholder" "placeholder"
    };

    monja::add(
        &sim.profile()?,
        sim.execution_options(),
        SetName("set1".into()),
        vec!["ripgrep".into()],
    )?;
    let result = monja::add(
        &sim.profile()?,
        sim.execution_options(),
        SetName("set2".into()),
        vec!["neovim".into(), "ripgrep".into()],
    )?;
    expect_that!(result.repo, eq(&RepoName::from("other")));

    let listed = monja::list(&sim.profile()?)?;
    expect_that!(listed.merged, elements_are![eq("ripgrep"), eq("neovim")]);

    Ok(())
}

#[gtest]
fn newset_creates_in_the_requested_repo() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");

    fs_operation! { LocalManipulation, sim,
        file "newfile" "newfile"
        file "otherfile" "otherfile"
    };

    let result = monja::new_set(
        &sim.profile()?,
        sim.execution_options(),
        Some(&monja::AbsolutePath::for_existing_path(sim.profile_path())?),
        vec![sim.local_path("newfile"), sim.local_path("otherfile")],
        SetName("newset".into()),
        Some(RepoName::from("other")),
    )?;
    expect_that!(result.repo, eq(&RepoName::from("other")));

    expect_that!(sim.set_path_in("other", "newset").exists(), is_true());
    expect_that!(sim.set_path_in("default", "newset").exists(), is_false());

    fs_operation! { RepoSetValidation, sim, "other", "newset",
        file "newfile" "newfile"
        file "otherfile" "otherfile"
    };

    Ok(())
}

// with several repos and no default configured, there's no defensible repo to pick, so the
// user has to say which one.
#[gtest]
fn newset_without_a_default_repo_errors() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");

    fs_operation! { LocalManipulation, sim,
        file "newfile" "newfile"
    };

    let result = monja::new_set(
        &sim.profile()?,
        sim.execution_options(),
        Some(&monja::AbsolutePath::for_existing_path(sim.profile_path())?),
        vec![sim.local_path("newfile")],
        SetName("newset".into()),
        None,
    );
    expect_that!(
        *result.unwrap_err(),
        pat!(NewSetError::RepoSelection(pat!(
            RepoSelectionError::NoDefault { .. }
        )))
    );

    Ok(())
}

#[gtest]
fn newset_uses_the_configured_default_repo() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");
    sim.configure_profile(|old| MonjaProfileConfig {
        default_repo: Some(RepoName::from("other")),
        ..old
    });

    fs_operation! { LocalManipulation, sim,
        file "newfile" "newfile"
        file "otherfile" "otherfile"
    };

    let result = monja::new_set(
        &sim.profile()?,
        sim.execution_options(),
        Some(&monja::AbsolutePath::for_existing_path(sim.profile_path())?),
        vec![sim.local_path("newfile"), sim.local_path("otherfile")],
        SetName("newset".into()),
        None,
    )?;
    expect_that!(result.repo, eq(&RepoName::from("other")));

    expect_that!(sim.set_path_in("other", "newset").exists(), is_true());

    Ok(())
}

#[gtest]
fn newset_refuses_a_name_that_exists_in_another_repo() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");

    fs_operation! { RepoSetManipulation, sim, "other", "taken",
        file "a" "a"
    };
    fs_operation! { LocalManipulation, sim,
        file "newfile" "newfile"
    };

    let result = monja::new_set(
        &sim.profile()?,
        sim.execution_options(),
        Some(&monja::AbsolutePath::for_existing_path(sim.profile_path())?),
        vec![sim.local_path("newfile")],
        SetName("taken".into()),
        Some(RepoName::from("default")),
    );
    let set_name = SetName("taken".into());
    expect_that!(
        *result.unwrap_err(),
        pat!(NewSetError::SetCreation(pat!(
            monja::SetCreationError::SetExists(&set_name)
        )))
    );

    Ok(())
}

// with exactly one repo, there's nothing to disambiguate, so no --repo and no default-repo
// should ever be needed. this is what keeps single-repo profiles working untouched.
#[gtest]
fn sole_repo_is_an_implicit_default() -> Result<()> {
    let sim = Simulator::create();

    let profile = sim.profile()?;
    let (name, _) = profile.resolve_repo(None)?;
    expect_that!(name, eq(&RepoName::default_name()));

    Ok(())
}

#[gtest]
fn unknown_repo_errors() -> Result<()> {
    let sim = Simulator::create();

    let profile = sim.profile()?;
    let result = profile.resolve_repo(Some(&RepoName::from("nosuchrepo")));
    expect_that!(result, err(pat!(RepoSelectionError::UnknownRepo { .. })));

    Ok(())
}

// the pre-multi-repo config form has to keep working, since existing profiles use it.
#[gtest]
fn legacy_repo_dir_still_works() -> Result<()> {
    let sim = Simulator::create();
    let repo_root = sim.repo_root().to_path_buf();
    sim.configure_profile(|old| MonjaProfileConfig {
        repo_dir: Some(repo_root),
        repos: Default::default(),
        target_sets: set_names(["simple"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "simple",
        file "afile" "afile"
    };

    let profile = sim.profile()?;
    // the unnamed single repo gets an implicit name so that --repo and error messages have
    // something to refer to
    expect_that!(
        profile.repo_names(),
        elements_are![eq(&RepoName::default_name())]
    );

    monja::pull(&profile, sim.execution_options())?;
    fs_operation! { LocalValidation, sim,
        file "afile" "afile"
    };

    Ok(())
}

#[gtest]
fn specifying_both_repo_dir_and_repos_errors() -> Result<()> {
    let sim = Simulator::create();
    let repo_root = sim.repo_root().to_path_buf();
    sim.configure_profile(|old| MonjaProfileConfig {
        repo_dir: Some(repo_root),
        ..old
    });

    expect_that!(
        sim.profile(),
        err(pat!(MonjaProfileConfigError::Profile(pat!(
            ProfileError::ConflictingRepoConfig
        ))))
    );

    Ok(())
}

#[gtest]
fn specifying_no_repos_errors() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        repo_dir: None,
        repos: Default::default(),
        ..old
    });

    expect_that!(
        sim.profile(),
        err(pat!(MonjaProfileConfigError::Profile(pat!(
            ProfileError::NoReposConfigured
        ))))
    );

    Ok(())
}

#[gtest]
fn unknown_default_repo_errors() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        default_repo: Some(RepoName::from("nosuchrepo")),
        ..old
    });

    expect_that!(
        sim.profile(),
        err(pat!(MonjaProfileConfigError::Profile(pat!(
            ProfileError::UnknownDefaultRepo { .. }
        ))))
    );

    Ok(())
}

// every repo has to be kept out of the local walk, or a repo living under $HOME would have its
// own contents treated as local files.
#[gtest]
fn all_repos_are_excluded_from_local_state() -> Result<()> {
    let mut sim = Simulator::create();
    sim.add_repo("other");
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["inother"]),
        ..old
    });

    fs_operation! { RepoSetManipulation, sim, "other", "inother",
        file "tracked" "tracked"
    };

    monja::pull(&sim.profile()?, sim.execution_options())?;

    let status = monja::local_status(&sim.profile()?, sim.cwd())?;
    // if the second repo weren't excluded, its set files would show up here as untracked
    expect_that!(status.untracked_files, is_empty());

    let profile = sim.profile()?;
    expect_that!(profile.repo_roots().count(), eq(2));

    Ok(())
}

// a quick sanity check that repo roots resolve relative to local root, since only the legacy
// form was previously exercised that way.
#[gtest]
fn relative_repo_dirs_resolve_against_local_root() -> Result<()> {
    let sim = Simulator::create();
    let relative = Path::new(sim.repo_root())
        .strip_prefix(sim.local_root())
        .unwrap()
        .to_path_buf();
    sim.configure_profile(|old| MonjaProfileConfig {
        repos: [(RepoName::default_name(), relative)].into_iter().collect(),
        target_sets: set_names(["simple"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "simple",
        file "afile" "afile"
    };

    let profile: MonjaProfile = sim.profile()?;
    monja::pull(&profile, sim.execution_options())?;

    fs_operation! { LocalValidation, sim,
        file "afile" "afile"
    };

    Ok(())
}
