use std::path::Path;

use googletest::prelude::*;
use monja::{CleanMode, MonjaProfileConfig};

use crate::sim::{Simulator, set_names};

#[allow(dead_code)]
#[macro_use]
mod sim;

#[gtest]
fn index_clean() -> Result<()> {
    let sim = Simulator::create();
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
fn index_clean_ignorefile() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        dir "foo"
            file "bar" "baz"
        end
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    fs_operation! { SetManipulation, sim, "set1",
        dir "foo"
            remfile "bar"
        end
    };

    let pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;
    expect_that!(pull_result.cleanable_files, { Path::new("foo/bar") });

    sim.configure_ignorefile("foo/bar");

    let clean_result = monja::clean(&sim.profile()?, sim.execution_options(), CleanMode::Index)?;
    expect_that!(clean_result.files_cleaned, is_empty());

    fs_operation! { LocalValidation, sim,
        dir "foo"
            file "bar" "baz"
        end
    };

    Ok(())
}

#[gtest]
fn full_clean() -> Result<()> {
    let sim = Simulator::create();
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

#[gtest]
fn full_clean_ignorefile() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        dir "foo"
            file "bar" "baz"
        end
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    fs_operation! { SetManipulation, sim, "set1",
        dir "foo"
            remfile "bar"
        end
    };

    let pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;
    expect_that!(pull_result.cleanable_files, { Path::new("foo/bar") });

    fs_operation! { LocalManipulation, sim,
        file "notinset" "notinset"
    };

    sim.configure_ignorefile("foo/bar\nnotinset");

    let clean_result = monja::clean(&sim.profile()?, sim.execution_options(), CleanMode::Index)?;
    expect_that!(clean_result.files_cleaned, is_empty());

    fs_operation! { LocalValidation, sim,
        file "notinset" "notinset"
        dir "foo"
            file "bar" "baz"
        end
    };

    Ok(())
}
