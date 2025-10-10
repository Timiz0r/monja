use googletest::prelude::*;

use crate::sim::{LocalValidation, SetManipulation, Simulator};
use monja::{MonjaProfileConfig, SetConfig, SetName};

#[allow(dead_code)]
#[macro_use]
mod sim;

// TODO: index verification
// TODO: special files excluded
// TODO: non-existing monjadir

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

    let _pull_result = monja::pull(&sim.profile())?;

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

    let _pull_result = monja::pull(&sim.profile())?;

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

    let _pull_result = monja::pull(&sim.profile())?;

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
        shortcut: Some("".into()),
    })
    .configure_set(SetName("set2".into()), |_| SetConfig {
        shortcut: Some(".config".into()),
    })
    .configure_set(SetName("set3".into()), |_| SetConfig {
        shortcut: Some(".config/myconfig".into()),
    });

    fs_operation! { SetManipulation, sim, "set1",
        dir ".config"
            dir "myconfig"
                file "foo" "set1"
            end
            file "blueberry" "tart1"
        end
    };
    fs_operation! { SetManipulation, sim, "set2",
        dir "myconfig"
            file "bar" "set2"
        end
        file "blueberry" "tart2"
    };
    fs_operation! { SetManipulation, sim, "set3",
        file "baz" "set3"
        file "bar" "set3"
    };

    let _pull_result = monja::pull(&sim.profile())?;

    fs_operation! { LocalValidation, sim,
        dir ".config"
            dir "myconfig"
                file "foo" "set1"
                file "bar" "set3"
                file "baz" "set3"
            end
            file "blueberry" "tart2"
        end
    };

    Ok(())
}

#[gtest]
fn directory_traversal() {}

#[gtest]
fn missing_set() {}

#[gtest]
fn missing_local_folder() {}

#[gtest]
fn missing_repo_folder() {}

fn set_names<S, N>(names: N) -> Vec<SetName>
where
    S: AsRef<str>,
    N: AsRef<[S]>,
{
    names
        .as_ref()
        .iter()
        .map(|n| SetName(n.as_ref().into()))
        .collect()
}
