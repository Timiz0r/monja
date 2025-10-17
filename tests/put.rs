use std::path::Path;

use googletest::prelude::*;
use monja::{MonjaProfileConfig, PushError, PutError, SetConfig, SetName};

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

    let put_result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("blueberry".as_ref())],
        SetName("set2".into()),
        true,
    )?;
    expect_that!(put_result.owning_set, pat!(SetName("set2")));
    expect_that!(put_result.files, { eq(Path::new("blueberry")) });

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

    let put_result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("blueberry".as_ref())],
        SetName("set2".into()),
        true,
    )?;
    expect_that!(put_result.owning_set, pat!(SetName("set2")));
    expect_that!(put_result.files, { eq(Path::new("blueberry")) });

    // succeeding is good enough
    let _push_result = monja::push(&sim.profile()?, sim.execution_options())?;

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
        file "blueberry" "tart"
    };
    fs_operation! { SetManipulation, sim, "set2",
    };

    let _pull_result = monja::pull(&sim.profile()?, sim.execution_options())?;

    sim.rem_set(SetName("set1".into()));

    let push_result = monja::push(&sim.profile()?, sim.execution_options());
    expect_that!(push_result, err(pat!(PushError::Consistency { .. })));

    sim.dryrun(true);
    let put_result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("blueberry".as_ref())],
        SetName("set2".into()),
        true,
    )?;
    expect_that!(put_result.owning_set, pat!(SetName("set2")));
    expect_that!(put_result.files, { eq(Path::new("blueberry")) });

    fs_operation! { SetValidation, sim, "set2",
        remfile "blueberry"
    };

    let status = monja::local_status(&sim.profile()?, sim.cwd())?;
    expect_that!(status.files_with_missing_sets, {
        (
            pat!(SetName("set1")),
            unordered_elements_are![eq(Path::new("blueberry"))],
        )
    });

    Ok(())
}

#[gtest]
fn no_index_update() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    })
    .configure_set(SetName("set1".into()), |_| SetConfig {
        // start with nested directory structure just in case
        shortcut: Some("foo/bar".into()),
    });

    fs_operation! { SetManipulation, sim, "set1",
    };

    fs_operation! { LocalManipulation, sim,
        dir "foo/bar"
            file "notinrepo" "notinrepo"
        end
    };

    let put_result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("foo/bar/notinrepo".as_ref())],
        SetName("set1".into()),
        false,
    )?;

    expect_that!(put_result.files, { Path::new("foo/bar/notinrepo") });
    expect_that!(put_result.owning_set, eq(&SetName("set1".into())));
    fs_operation! { SetValidation, sim, "set1",
        file "notinrepo" "notinrepo"
    };

    let status = monja::local_status(&sim.profile()?, sim.cwd())?;
    expect_that!(status.untracked_files, {
        eq(Path::new("foo/bar/notinrepo"))
    });

    Ok(())
}

#[gtest]
fn nonexistent_set() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
    };

    fs_operation! { LocalManipulation, sim,
        file "notinrepo" "notinrepo"
    };

    let put_result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("notinrepo".as_ref())],
        SetName("set2".into()),
        false,
    );
    expect_that!(
        put_result,
        err(pat!(PutError::SetNotFound(&SetName("set2".into()))))
    );

    Ok(())
}

#[gtest]
fn nonexistent_file() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
    };

    let put_result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("notinlocal".as_ref())],
        SetName("set1".into()),
        false,
    );
    expect_that!(
        put_result,
        err(pat!(PutError::NotValidFile(
            &sim.local_root().join("notinlocal")
        )))
    );

    Ok(())
}

#[gtest]
fn shortcut() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    })
    .configure_set(SetName("set1".into()), |_| SetConfig {
        // start with nested directory structure just in case
        shortcut: Some("foo/bar".into()),
    });

    fs_operation! { SetManipulation, sim, "set1",
    };

    fs_operation! { LocalManipulation, sim,
        dir "foo/bar"
            file "notinrepo" "notinrepo"
        end
    };

    let put_result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("foo/bar/notinrepo".as_ref())],
        SetName("set1".into()),
        false,
    )?;

    expect_that!(put_result.files, { Path::new("foo/bar/notinrepo") });
    expect_that!(put_result.owning_set, eq(&SetName("set1".into())));
    fs_operation! { SetValidation, sim, "set1",
        file "notinrepo" "notinrepo"
    };

    Ok(())
}

#[gtest]
fn path_outside_of_shortcut() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    })
    .configure_set(SetName("set1".into()), |_| SetConfig {
        // start with nested directory structure just in case
        shortcut: Some("foo/bar".into()),
    });

    fs_operation! { SetManipulation, sim, "set1",
    };

    fs_operation! { LocalManipulation, sim,
        file "notinrepo" "notinrepo"
    };

    let put_result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("notinrepo".as_ref())],
        SetName("set1".into()),
        false,
    );
    expect_that!(put_result, err(pat!(PutError::SetPath(..))));

    Ok(())
}

#[gtest]
fn only_in_pushed_set() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
    };

    fs_operation! { LocalManipulation, sim,
        file "notinrepo" "notinrepo"
    };

    let put_result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("notinrepo".as_ref())],
        SetName("set1".into()),
        false,
    )?;

    expect_that!(put_result.untracked_files, is_empty());
    expect_that!(put_result.files_in_later_sets, is_empty());
    fs_operation! { SetValidation, sim, "set1",
        file "notinrepo" "notinrepo"
    };

    Ok(())
}

#[gtest]
fn untracked_files() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
    };
    fs_operation! { SetManipulation, sim, "set2",
    };

    fs_operation! { LocalManipulation, sim,
        file "notinrepo" "notinrepo"
    };

    let put_result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("notinrepo".as_ref())],
        SetName("set1".into()),
        false,
    )?;

    expect_that!(put_result.untracked_files, { Path::new("notinrepo") });
    expect_that!(put_result.files_in_later_sets, is_empty());
    fs_operation! { SetValidation, sim, "set1",
        file "notinrepo" "notinrepo"
    };

    Ok(())
}

#[gtest]
fn files_in_later_sets() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2", "set3"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
    };
    fs_operation! { SetManipulation, sim, "set2",
        file "notinrepo" "notinrepo"
    };
    fs_operation! { SetManipulation, sim, "set3",
        file "notinrepo" "notinrepo"
    };

    fs_operation! { LocalManipulation, sim,
        file "notinrepo" "notinrepo"
    };

    let put_result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("notinrepo".as_ref())],
        SetName("set1".into()),
        false,
    )?;

    expect_that!(put_result.untracked_files, is_empty());
    expect_that!(put_result.files_in_later_sets, {
        (
            eq(Path::new("notinrepo")),
            unordered_elements_are![pat!(SetName("set2")), pat!(SetName("set3"))],
        )
    });
    fs_operation! { SetValidation, sim, "set1",
        file "notinrepo" "notinrepo"
    };

    Ok(())
}

#[gtest]
fn ignores_ignore_file() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
    };

    fs_operation! { LocalManipulation, sim,
        file "notinrepo" "notinrepo"
    };

    sim.configure_ignorefile("notinrepo");

    let put_result = monja::put(
        &sim.profile()?,
        sim.execution_options(),
        vec![sim.local_path("notinrepo".as_ref())],
        SetName("set1".into()),
        false,
    )?;
    expect_that!(put_result.files, len(eq(1)));

    fs_operation! { SetValidation, sim, "set1",
        file "notinrepo" "notinrepo"
    };

    Ok(())
}
