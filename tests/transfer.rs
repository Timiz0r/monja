use std::path::Path;

use googletest::prelude::*;
use monja::{MonjaProfileConfig, SetConfig, SetName, TransferError};

use crate::sim::{Simulator, set_names};

#[allow(dead_code)]
#[macro_use]
mod sim;

#[gtest]
fn basic_move() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "apple" "pie"
        file "blueberry" "tart"
    };
    fs_operation! { SetManipulation, sim, "set2",
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    let result = monja::transfer(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("apple")],
        SetName("set1".into()),
        SetName("set2".into()),
    )?;

    expect_that!(result.source_set, pat!(SetName("set1")));
    expect_that!(result.dest_set, pat!(SetName("set2")));
    expect_that!(result.files, { eq(Path::new("apple")) });

    fs_operation! { SetValidation, sim, "set2",
        file "apple" "pie"
    };
    fs_operation! { SetValidation, sim, "set1",
        file "blueberry" "tart"
        remfile "apple"
    };

    Ok(())
}

#[gtest]
fn move_updates_index() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "apple" "pie"
    };
    fs_operation! { SetManipulation, sim, "set2",
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    let _result = monja::transfer(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("apple")],
        SetName("set1".into()),
        SetName("set2".into()),
    )?;

    let status = monja::local_status(&sim.profile()?, sim.cwd())?;
    expect_that!(status.files_to_push, {
        (
            pat!(SetName("set2")),
            unordered_elements_are![eq(Path::new("apple"))],
        )
    });

    Ok(())
}

#[gtest]
fn source_set_not_found() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "apple" "pie"
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    let result = monja::transfer(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("apple")],
        SetName("nonexistent".into()),
        SetName("set1".into()),
    );
    expect_that!(
        result,
        err(pat!(TransferError::SourceSetNotFound(&SetName(
            "nonexistent".into()
        ))))
    );

    Ok(())
}

#[gtest]
fn dest_set_not_found() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "apple" "pie"
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    let result = monja::transfer(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("apple")],
        SetName("set1".into()),
        SetName("nonexistent".into()),
    );
    expect_that!(
        result,
        err(pat!(TransferError::DestSetNotFound(&SetName(
            "nonexistent".into()
        ))))
    );

    Ok(())
}

#[gtest]
fn file_not_in_source_set() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
    };
    fs_operation! { SetManipulation, sim, "set2",
    };

    fs_operation! { LocalManipulation, sim,
        file "notinset" "notinset"
    };

    let result = monja::transfer(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("notinset")],
        SetName("set1".into()),
        SetName("set2".into()),
    );
    expect_that!(result, err(pat!(TransferError::NotInSourceSet { .. })));

    Ok(())
}

#[gtest]
fn dest_shortcut_incompatible() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    })
    .configure_set(SetName("set2".into()), |_| SetConfig {
        shortcut: Some("other/prefix".into()),
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "apple" "pie"
    };
    fs_operation! { SetManipulation, sim, "set2",
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    let result = monja::transfer(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("apple")],
        SetName("set1".into()),
        SetName("set2".into()),
    );
    expect_that!(result, err(pat!(TransferError::DestSetPath(..))));

    fs_operation! { SetValidation, sim, "set1",
        file "apple" "pie"
    };

    Ok(())
}

#[gtest]
fn dryrun() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "apple" "pie"
    };
    fs_operation! { SetManipulation, sim, "set2",
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    sim.dryrun(true);
    let result = monja::transfer(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("apple")],
        SetName("set1".into()),
        SetName("set2".into()),
    )?;

    expect_that!(result.files, { eq(Path::new("apple")) });

    fs_operation! { SetValidation, sim, "set1",
        file "apple" "pie"
    };
    fs_operation! { SetValidation, sim, "set2",
        remfile "apple"
    };

    Ok(())
}

#[gtest]
fn move_with_shortcut() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    })
    .configure_set(SetName("set1".into()), |_| SetConfig {
        shortcut: Some("foo/bar".into()),
    })
    .configure_set(SetName("set2".into()), |_| SetConfig {
        shortcut: Some("foo/bar".into()),
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "baz" "baz"
    };
    fs_operation! { SetManipulation, sim, "set2",
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    let result = monja::transfer(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("foo/bar/baz")],
        SetName("set1".into()),
        SetName("set2".into()),
    )?;

    expect_that!(result.files, { eq(Path::new("foo/bar/baz")) });
    fs_operation! { SetValidation, sim, "set2",
        file "baz" "baz"
    };
    fs_operation! { SetValidation, sim, "set1",
        remfile "baz"
    };

    Ok(())
}
