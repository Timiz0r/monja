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

    // old index will have foo/bar
    let _ = monja::pull(&sim.profile()?, sim.execution_options())?;

    fs_operation! { SetManipulation, sim, "set1",
        dir "foo"
            remfile "bar"
        end
        file ".monjaignore" "foo/bar"
    };

    fs_operation! { LocalManipulation, sim,
        file "notignored" "notignored"
    };

    // new index doesn't have foo/bar, so should be eligible for index clean if not for ignore
    let pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;
    expect_that!(pull_result.cleanable_files, is_empty());

    let clean_result = monja::clean(&sim.profile()?, sim.execution_options(), CleanMode::Index)?;
    expect_that!(clean_result.files_cleaned, is_empty());

    fs_operation! { LocalValidation, sim,
        dir "foo"
            file "bar" "baz"
        end
        file "notignored" "notignored"
        file ".monjaignore" "foo/bar"
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
        file ".monjaignore" "foo/bar"
    };

    _ = monja::pull(&sim.profile()?, sim.execution_options())?;

    fs_operation! { LocalManipulation, sim,
        dir "foo"
            file "bar" "baz"
        end
        file "notignored" "notignored"
    };

    let pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;
    expect_that!(pull_result.cleanable_files, is_empty());

    let clean_result = monja::clean(&sim.profile()?, sim.execution_options(), CleanMode::Full)?;
    expect_that!(clean_result.files_cleaned, { eq(Path::new("notignored")) });

    fs_operation! { LocalValidation, sim,
        dir "foo"
            file "bar" "baz"
        end
        file ".monjaignore" "foo/bar"
    };

    Ok(())
}

#[gtest]
fn index_clean_dryrun() -> Result<()> {
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

    sim.dryrun(true);
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
        file "set1a" "set1a-pull1"
        file "set1b" "set1b-pull1"
        file "set2a" "set2a-pull1"
        file "set2b" "set2b-pull2"
        file "notinrepo" "notinrepo"
    };

    Ok(())
}

#[gtest]
fn full_clean_dryrun() -> Result<()> {
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

    sim.dryrun(true);
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
        file "set1a" "set1a-pull1"
        file "set1b" "set1b-pull1"
        file "set2a" "set2a-pull1"
        file "set2b" "set2b-pull2"
        file "notinrepo" "notinrepo"
    };

    Ok(())
}
