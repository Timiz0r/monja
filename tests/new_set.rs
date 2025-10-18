use std::path::Path;

use googletest::prelude::*;
use monja::{AbsolutePath, NewSetError, SetCreationError, SetName};

use crate::sim::Simulator;

#[allow(dead_code)]
#[macro_use]
mod sim;

#[gtest]
fn basic() -> Result<()> {
    let sim = Simulator::create();

    fs_operation! { LocalManipulation, sim,
        file "notinrepo" "notinrepo"
        file "alsonotinrepo" "alsonotinrepo"
    };

    let new_set_result = monja::new_set(
        &sim.profile()?,
        sim.execution_options(),
        &AbsolutePath::for_existing_path(sim.profile_path())?,
        vec![sim.local_path("notinrepo"), sim.local_path("alsonotinrepo")],
        SetName("newset".into()),
    )?;
    expect_that!(new_set_result.new_set, pat!(SetName("newset")));
    expect_that!(new_set_result.files, { eq(Path::new("notinrepo")), eq(Path::new("alsonotinrepo")) });

    fs_operation! { SetValidation, sim, "newset",
        file "notinrepo" "notinrepo"
        file "alsonotinrepo" "alsonotinrepo"
    };

    let status = monja::local_status(&sim.profile()?, sim.cwd())?;
    expect_that!(status.files_to_push, {
        (
            pat!(SetName("newset")),
            unordered_elements_are![eq(Path::new("notinrepo")), eq(Path::new("alsonotinrepo"))],
        )
    });

    Ok(())
}

#[gtest]
fn common_prefix() -> Result<()> {
    let sim = Simulator::create();

    fs_operation! { LocalManipulation, sim,
        dir "a/b/c"
            file "1" "1"
            file "2" "2"
            dir "d"
                file "3" "3"
            end
        end
    };

    let new_set_result = monja::new_set(
        &sim.profile()?,
        sim.execution_options(),
        &AbsolutePath::for_existing_path(sim.profile_path())?,
        vec![
            sim.local_path("a/b/c/1"),
            sim.local_path("a/b/c/2"),
            sim.local_path("a/b/c/d/3"),
        ],
        SetName("newset".into()),
    )?;
    expect_that!(new_set_result.new_set, pat!(SetName("newset")));
    expect_that!(new_set_result.files, {
        eq(Path::new("a/b/c/1")),
        eq(Path::new("a/b/c/2")),
        eq(Path::new("a/b/c/d/3"))
    });

    // the important one
    expect_that!(sim.repo_root().join("newset/1").exists(), is_true());
    expect_that!(sim.repo_root().join("newset/2").exists(), is_true());
    expect_that!(sim.repo_root().join("newset/d/3").exists(), is_true());

    fs_operation! { SetValidation, sim, "newset",
            file "1" "1"
            file "2" "2"
            dir "d"
                file "3" "3"
            end
    };

    let status = monja::local_status(&sim.profile()?, sim.cwd())?;
    expect_that!(status.files_to_push, {
        (
            pat!(SetName("newset")),
            unordered_elements_are![
                eq(Path::new("a/b/c/1")),
                eq(Path::new("a/b/c/2")),
                eq(Path::new("a/b/c/d/3"))
            ],
        )
    });

    Ok(())
}

#[gtest]
fn dryrun() -> Result<()> {
    let mut sim = Simulator::create();

    fs_operation! { LocalManipulation, sim,
        file "notinrepo" "notinrepo"
        file "alsonotinrepo" "alsonotinrepo"
    };

    sim.dryrun(true);
    let new_set_result = monja::new_set(
        &sim.profile()?,
        sim.execution_options(),
        &AbsolutePath::for_existing_path(sim.profile_path())?,
        vec![sim.local_path("notinrepo"), sim.local_path("alsonotinrepo")],
        SetName("newset".into()),
    )?;
    expect_that!(new_set_result.new_set, pat!(SetName("newset")));
    expect_that!(new_set_result.files, { eq(Path::new("notinrepo")), eq(Path::new("alsonotinrepo")) });

    // the normal SetValidation stuff doesn't have a way to verify a set doesn't exist
    expect_that!(sim.repo_root().join("newset").exists(), is_false());

    let status = monja::local_status(&sim.profile()?, sim.cwd())?;
    expect_that!(status.untracked_files, {
        Path::new("notinrepo"),
        Path::new("alsonotinrepo")
    });

    Ok(())
}

#[gtest]
fn set_exists() -> Result<()> {
    let sim = Simulator::create();

    fs_operation! { SetManipulation, sim, "newset",
        file "notinrepo" "notinrepo"
        file "alsonotinrepo" "alsonotinrepo"
    };

    let new_set_result = monja::new_set(
        &sim.profile()?,
        sim.execution_options(),
        &AbsolutePath::for_existing_path(sim.profile_path())?,
        vec![sim.local_path("notinrepo"), sim.local_path("alsonotinrepo")],
        SetName("newset".into()),
    );

    let set_name = SetName("newset".into());
    let specific_error = pat!(SetCreationError::SetExists(&set_name));
    expect_that!(
        *new_set_result.unwrap_err(),
        pat!(NewSetError::SetCreation(specific_error))
    );

    Ok(())
}
