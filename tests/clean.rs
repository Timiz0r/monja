use std::path::Path;

use googletest::prelude::*;
use monja::{CleanMode, MonjaProfileConfig};

use crate::sim::{Simulator, set_names};

#[allow(dead_code)]
#[macro_use]
mod sim;

#[gtest]
fn index_clean() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "set1a" "set1a-pull1"
        file "set1b" "set1b-pull1"
    };
    fs_operation! { SetManipulation, sim, "set2",
        file "set2a" "set2a-pull1"
        file "set2b" "set2b-pull1"
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "set1a" "set1a-pull2"
        file "set1b" "set1b-pull2"
    };
    fs_operation! { SetManipulation, sim, "set2",
        remfile "set2a"
        file "set2b" "set2b-pull2"
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;
    fs_operation! { LocalManipulation, sim,
        file "notinrepo" "notinrepo"
    };
    let clean_result = monja::clean(&sim.profile()?, sim.execution_options(), CleanMode::Index)?;
    expect_that!(
        clean_result.files_cleaned,
        {
            eq(Path::new("set1a")),
            eq(Path::new("set1b")),
            eq(Path::new("set2a"))
        }
    );

    fs_operation! { LocalValidation, sim,
        file "set2b" "set2b-pull2"
        file "notinrepo" "notinrepo"
    };

    Ok(())
}

#[gtest]
fn full_clean() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "set1a" "set1a-pull1"
        file "set1b" "set1b-pull1"
    };
    fs_operation! { SetManipulation, sim, "set2",
        file "set2a" "set2a-pull1"
        file "set2b" "set2b-pull1"
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "set1a" "set1a-pull2"
        file "set1b" "set1b-pull2"
    };
    fs_operation! { SetManipulation, sim, "set2",
        remfile "set2a"
        file "set2b" "set2b-pull2"
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;
    fs_operation! { LocalManipulation, sim,
        file "notinrepo" "notinrepo"
    };
    let clean_result = monja::clean(&sim.profile()?, sim.execution_options(), CleanMode::Full)?;
    expect_that!(
        clean_result.files_cleaned,
        {
            eq(Path::new("set1a")),
            eq(Path::new("set1b")),
            eq(Path::new("set2a")),
            eq(Path::new("notinrepo"))
        }
    );

    fs_operation! { LocalValidation, sim,
        file "set2b" "set2b-pull2"
    };

    Ok(())
}
