use std::path::Path;

use googletest::prelude::*;

use monja::{MonjaProfileConfig, SetName};

use crate::sim::{Simulator, set_names};

#[allow(dead_code)]
#[macro_use]
mod sim;

// testing all in one go just to ensure everything gets populated correctly all in one go

#[gtest]
fn status() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "set1" "set1"
    };

    fs_operation! { SetManipulation, sim, "set2",
        file "set2a" "set2a"
        file "set2b" "set2b"
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    sim.rem_set(SetName("set1".into()));
    fs_operation! { SetManipulation, sim, "set2",
        remfile "set2b"
    };
    fs_operation! { LocalManipulation, sim,
        file "notinrepo" "notinrepo"
    };

    let status = monja::local_status(&sim.profile()?)?;
    expect_that!(status.files_to_push, {
        (
            pat!(SetName("set2")),
            unordered_elements_are![eq(Path::new("set2a"))],
        )
    });
    expect_that!(status.files_with_missing_sets, {
        (
            pat!(SetName("set1")),
            unordered_elements_are![eq(Path::new("set1"))],
        )
    });
    expect_that!(status.missing_files, {
        (
            pat!(SetName("set2")),
            unordered_elements_are![eq(Path::new("set2b"))],
        )
    });
    expect_that!(status.untracked_files, { eq(Path::new("notinrepo")) });

    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set2"]),
        ..old
    });
    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;
    let status = monja::local_status(&sim.profile()?)?;
    expect_that!(
        status.untracked_files,
        {
            eq(Path::new("notinrepo")),
            eq(Path::new("set1")),
            eq(Path::new("set2b"))
        }
    );
    expect_that!(
        status.old_files_since_last_pull,
        {
            eq(Path::new("set1")),
            eq(Path::new("set2b"))
        }
    );

    Ok(())
}
