use std::path::Path;

use googletest::prelude::*;
use monja::{MonjaProfileConfig, SetConfig, SetName};

use crate::sim::{Simulator, set_names};

#[allow(dead_code)]
#[macro_use]
mod sim;

#[gtest]
fn basic_change() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["myset"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "myset",
        dir "foo"
            file "bar.conf" "hello"
            file "baz.conf" "world"
        end
    };
    sim.configure_set(SetName("myset".into()), |_| SetConfig {
        shortcut: Some(".config".into()),
    });

    let _pull = monja::pull(&sim.profile()?, sim.execution_options())?;

    let result = monja::set_shortcut(
        &sim.profile()?,
        sim.execution_options(),
        SetName("myset".into()),
        ".config/foo".into(),
    )?;

    expect_that!(result.set_name, pat!(SetName("myset")));
    expect_that!(result.old_shortcut, eq(Path::new(".config")));
    expect_that!(result.new_shortcut, eq(Path::new(".config/foo")));
    expect_that!(
        result.files_moved,
        unordered_elements_are![eq(Path::new("bar.conf")), eq(Path::new("baz.conf"))]
    );

    fs_operation! { SetValidation, sim, "myset",
        file "bar.conf" "hello"
        file "baz.conf" "world"
    };

    let config = SetConfig::load(&sim.profile()?, &SetName("myset".into()))?;
    expect_that!(config.shortcut, some(eq(Path::new(".config/foo"))));

    Ok(())
}

#[gtest]
fn widen_shortcut() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["myset"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "myset",
        file "bar.conf" "hello"
        file "baz.conf" "world"
    };
    sim.configure_set(SetName("myset".into()), |_| SetConfig {
        shortcut: Some(".config/foo".into()),
    });

    let _pull = monja::pull(&sim.profile()?, sim.execution_options())?;

    let result = monja::set_shortcut(
        &sim.profile()?,
        sim.execution_options(),
        SetName("myset".into()),
        ".config".into(),
    )?;

    expect_that!(result.files_moved.len(), eq(2));

    fs_operation! { SetValidation, sim, "myset",
        file "foo/bar.conf" "hello"
        file "foo/baz.conf" "world"
    };

    Ok(())
}

#[gtest]
fn remove_shortcut() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["myset"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "myset",
        file "bar.conf" "hello"
    };
    sim.configure_set(SetName("myset".into()), |_| SetConfig {
        shortcut: Some(".config".into()),
    });

    let _pull = monja::pull(&sim.profile()?, sim.execution_options())?;

    // remove shortcut entirely
    let result = monja::set_shortcut(
        &sim.profile()?,
        sim.execution_options(),
        SetName("myset".into()),
        "".into(),
    )?;

    expect_that!(result.files_moved.len(), eq(1));

    fs_operation! { SetValidation, sim, "myset",
        file ".config/bar.conf" "hello"
    };

    let config = SetConfig::load(&sim.profile()?, &SetName("myset".into()))?;
    expect_that!(config.shortcut, none());

    Ok(())
}

#[gtest]
fn file_outside_new_shortcut() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["myset"]),
        ..old
    });

    // files under .config and .local
    fs_operation! { SetManipulation, sim, "myset",
        dir ".config"
            file "foo.conf" "a"
        end
        dir ".local"
            file "bar.conf" "b"
        end
    };
    // no shortcut, so these are stored as-is

    let _pull = monja::pull(&sim.profile()?, sim.execution_options())?;

    // try to set shortcut to .config — .local/bar.conf can't fit
    let result = monja::set_shortcut(
        &sim.profile()?,
        sim.execution_options(),
        SetName("myset".into()),
        ".config".into(),
    );

    expect_that!(result, err(anything()));

    fs_operation! { SetValidation, sim, "myset",
        dir ".config"
            file "foo.conf" "a"
        end
        dir ".local"
            file "bar.conf" "b"
        end
    };

    Ok(())
}

#[gtest]
fn set_not_found() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["myset"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "myset",
        file "foo" "bar"
    };

    let result = monja::set_shortcut(
        &sim.profile()?,
        sim.execution_options(),
        SetName("nonexistent".into()),
        ".config".into(),
    );

    expect_that!(result, err(anything()));

    Ok(())
}

#[gtest]
fn dryrun() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["myset"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "myset",
        dir "foo"
            file "bar.conf" "hello"
        end
    };
    sim.configure_set(SetName("myset".into()), |_| SetConfig {
        shortcut: Some(".config".into()),
    });

    let _pull = monja::pull(&sim.profile()?, sim.execution_options())?;

    sim.dryrun(true);
    let result = monja::set_shortcut(
        &sim.profile()?,
        sim.execution_options(),
        SetName("myset".into()),
        ".config/foo".into(),
    )?;

    expect_that!(result.files_moved.len(), eq(1));

    // files should NOT have moved
    fs_operation! { SetValidation, sim, "myset",
        dir "foo"
            file "bar.conf" "hello"
        end
    };

    let config = SetConfig::load(&sim.profile()?, &SetName("myset".into()))?;
    expect_that!(config.shortcut, some(eq(Path::new(".config"))));

    Ok(())
}

#[gtest]
fn same_shortcut_noop() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["myset"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "myset",
        file "foo.conf" "hello"
    };
    sim.configure_set(SetName("myset".into()), |_| SetConfig {
        shortcut: Some(".config".into()),
    });

    let _pull = monja::pull(&sim.profile()?, sim.execution_options())?;

    let result = monja::set_shortcut(
        &sim.profile()?,
        sim.execution_options(),
        SetName("myset".into()),
        ".config".into(),
    )?;

    expect_that!(result.files_moved.len(), eq(0));

    fs_operation! { SetValidation, sim, "myset",
        file "foo.conf" "hello"
    };

    Ok(())
}
