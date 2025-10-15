use googletest::prelude::*;
use monja::{MonjaProfileConfig, PushError, SetName};

use crate::sim::{Simulator, set_names};

#[allow(dead_code)]
#[macro_use]
mod sim;

#[gtest]
fn fix_missing_set() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "blueberry" "tart"
    };
    fs_operation! { SetManipulation, sim, "set2",
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    sim.rem_set(SetName("set1".into()));

    let push_result = monja::push(&sim.profile()?, sim.execution_options());
    expect_that!(push_result, err(pat!(PushError::Consistency { .. })));

    let _fix_result = monja::fix(
        &sim.profile()?,
        sim.execution_options(),
        &[sim.local_file_path("blueberry".as_ref())],
        SetName("set2".into()),
    )?;

    // succeeding is good enough
    let _push_result = monja::push(&sim.profile()?, sim.execution_options())?;

    Ok(())
}

#[gtest]
fn fix_missing_files() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "blueberry" "tart"
        file "apple" "pie"
    };
    fs_operation! { SetManipulation, sim, "set2",
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    fs_operation! { SetManipulation, sim, "set1",
        remfile "blueberry"
    };

    let push_result = monja::push(&sim.profile()?, sim.execution_options());
    expect_that!(push_result, err(pat!(PushError::Consistency { .. })));

    let _fix_result = monja::fix(
        &sim.profile()?,
        sim.execution_options(),
        &[sim.local_file_path("blueberry".as_ref())],
        SetName("set2".into()),
    )?;

    // succeeding is good enough
    let _push_result = monja::push(&sim.profile()?, sim.execution_options())?;

    Ok(())
}
