use crate::sim::{LocalValidation, SetManipulation, Simulator};
use monja::{MonjaProfile, MonjaProfileConfig, SetConfig, SetName};

use googletest::prelude::*;

#[allow(dead_code)]
#[macro_use]
mod sim;

// TODO: index verification
// TODO: special files excluded

#[gtest]
fn simple_set() {
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

    monja::pull(&sim.profile());

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
}

#[gtest]
fn multiple_sets() {
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

    monja::pull(&sim.profile());

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

    monja::pull(&sim.profile());

    fs_operation! { LocalValidation, sim,
        dir "foo"
            dir "bar"
                file "baz" "set1baz"
            end
        end
        file "set1only" "set1only"
        file "set2only" "set2only"
    };
}

#[gtest]
fn shortcuts() {
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

    monja::pull(&sim.profile());

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
}

#[gtest]
fn multiple_sets_different_shortcuts_same_local_files() {}

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
