use googletest::prelude::*;

use crate::sim::{Simulator, set_names};
use monja::{
    AbsolutePath, MonjaProfile, MonjaProfileConfig, PullError, RepoStateInitializationError,
    SetConfig, SetName,
};

#[allow(dead_code)]
#[macro_use]
mod sim;

#[gtest]
fn simple_set() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["simple"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "simple",
        dir "foo"
            dir "bar/baz"
                file "cake" "cake"
            end
        end
        dir "apple"
            file "pie" "pie"
            file "pasta" "pasta"
        end
        file "blueberry" "tart"
    };

    let _pull_result = monja::pull(&sim.profile()?)?;

    fs_operation! { LocalValidation, sim,
        dir "foo"
            dir "bar/baz"
                file "cake" "cake"
            end
        end
        dir "apple"
            file "pie" "pie"
            file "pasta" "pasta"
        end
        file "blueberry" "tart"
    };

    Ok(())
}

#[gtest]
fn multiple_sets() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        dir "foo"
            dir "bar"
                file "baz" "set1baz"
            end
        end
        file "set1only" "set1only"
    };
    fs_operation! { SetManipulation, sim, "set2",
        dir "foo"
            dir "bar"
                file "baz" "set2baz"
            end
        end
        file "set2only" "set2only"
    };

    let _pull_result = monja::pull(&sim.profile()?)?;

    fs_operation! { LocalValidation, sim,
        dir "foo"
            dir "bar"
                file "baz" "set2baz"
            end
        end
        file "set1only" "set1only"
        file "set2only" "set2only"
    };

    // reverse!
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set2", "set1"]),
        ..old
    });

    let _pull_result = monja::pull(&sim.profile()?)?;

    fs_operation! { LocalValidation, sim,
        dir "foo"
            dir "bar"
                file "baz" "set1baz"
            end
        end
        file "set1only" "set1only"
        file "set2only" "set2only"
    };

    Ok(())
}

#[gtest]
fn shortcuts() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2", "set3"]),
        ..old
    })
    .configure_set(SetName("set1".into()), |_| SetConfig {
        // start with nested directory structure just in case
        shortcut: Some(".config/myconfig".into()),
    })
    .configure_set(SetName("set2".into()), |_| SetConfig {
        shortcut: Some(".config".into()),
    })
    .configure_set(SetName("set3".into()), |_| SetConfig {
        shortcut: Some("".into()),
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "baz" "set1"
        file "bar" "set1"
    };
    fs_operation! { SetManipulation, sim, "set2",
        dir "myconfig"
            file "foo" "set2"
        end
        file "blueberry" "tart2"
    };
    fs_operation! { SetManipulation, sim, "set3",
        dir ".config"
            dir "myconfig"
                file "bar" "set3"
            end
            file "blueberry" "tart3"
        end
    };

    let _pull_result = monja::pull(&sim.profile()?)?;

    fs_operation! { LocalValidation, sim,
        dir ".config"
            dir "myconfig"
                file "foo" "set2"
                file "bar" "set3"
                file "baz" "set1"
            end
            file "blueberry" "tart3"
        end
    };

    Ok(())
}

#[gtest]
fn shorcut_directory_traversal() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    })
    .configure_set(SetName("set1".into()), |_| SetConfig {
        shortcut: Some("..".into()),
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "foo" "set1"
    };

    let result = monja::pull(&sim.profile()?);
    let specific_error = contains(pat!(RepoStateInitializationError::SetShortcutInvalid(
        pat!(monja::SetShortcutError::TraversalToParent(..))
    )));
    expect_that!(
        result,
        err(pat!(PullError::RepoStateInitialization(specific_error)))
    );
    Ok(())
}

#[gtest]
fn shorcut_absolute_path() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    })
    .configure_set(SetName("set1".into()), |_| SetConfig {
        shortcut: Some("/".into()),
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "foo" "set1"
    };

    let result = monja::pull(&sim.profile()?);
    let specific_error = contains(pat!(RepoStateInitializationError::SetShortcutInvalid(
        pat!(monja::SetShortcutError::NotRelative(..))
    )));
    expect_that!(
        result,
        err(pat!(PullError::RepoStateInitialization(specific_error)))
    );
    Ok(())
}

#[gtest]
fn missing_set() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        dir "foo"
            dir "bar"
                file "baz" "set1baz"
            end
        end
        file "set1only" "set1only"
    };
    let result = monja::pull(&sim.profile()?);
    expect_that!(
        result,
        err(pat!(PullError::MissingSets(contains(eq(&SetName(
            "set2".into()
        ))))))
    );

    Ok(())
}

// #[gtest]
// fn missing_local_folder() -> Result<()> {
//     // this test case realistically does not exist.
//     // first, the home folder should exist.
//     // second, if it doesnt or the profile doesn't exist, the main function will fail, not monja::push.
//     Ok(())
// }

#[gtest]
fn missing_repo_folder() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["simple"]),
        ..old
    });

    let repo_root;
    {
        let new_root = tempfile::Builder::new()
            .prefix("NonExistingMonjaRepo")
            .tempdir()?;
        repo_root = AbsolutePath::for_existing_path(new_root.path())?;
        // drop tempdir, which deletes it. gotta go thru hoops because AbsolutePath::for_existing_path
    }

    let profile = MonjaProfile {
        repo_root,
        ..sim.profile()?
    };
    let result = monja::pull(&profile);
    let specific_error = contains(pat!(RepoStateInitializationError::Io(..)));
    expect_that!(
        result,
        err(pat!(PullError::RepoStateInitialization(specific_error)))
    );

    Ok(())
}

#[gtest]
fn set_with_empty_name() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["", "set1"]),
        ..old
    });

    fs_operation! { SetManipulation, sim, "set1",
        file "foo" "set1"
    };

    let result = monja::pull(&sim.profile()?);
    expect_that!(
        result,
        err(pat!(PullError::MissingSets(container_eq(set_names([""])))))
    );

    Ok(())
}
