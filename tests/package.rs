use googletest::prelude::*;
use monja::{MonjaProfileConfig, SetConfig, SetName};

use crate::sim::{Simulator, set_names};

#[allow(dead_code)]
#[macro_use]
mod sim;

#[gtest]
fn add_new_and_already_present() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["myset"]),
        ..old
    });
    fs_operation! { SetManipulation, sim, "myset",
        file "placeholder" "x"
    };

    let result = monja::add(
        &sim.profile()?,
        sim.execution_options(),
        SetName("myset".into()),
        vec!["git".into(), "neovim".into()],
    )?;

    expect_that!(
        result.added,
        unordered_elements_are![eq("git"), eq("neovim")]
    );
    expect_that!(result.already_present, is_empty());

    let result = monja::add(
        &sim.profile()?,
        sim.execution_options(),
        SetName("myset".into()),
        vec!["git".into(), "ripgrep".into()],
    )?;

    expect_that!(result.added, unordered_elements_are![eq("ripgrep")]);
    expect_that!(result.already_present, unordered_elements_are![eq("git")]);

    let config = SetConfig::load(&sim.profile()?, &SetName("myset".into()))?;
    expect_that!(
        config.packages,
        unordered_elements_are![eq("git"), eq("neovim"), eq("ripgrep")]
    );

    Ok(())
}

#[gtest]
fn add_set_not_found() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["myset"]),
        ..old
    });

    let result = monja::add(
        &sim.profile()?,
        sim.execution_options(),
        SetName("nonexistent".into()),
        vec!["git".into()],
    );

    expect_that!(result, err(anything()));

    Ok(())
}

#[gtest]
fn add_dry_run() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["myset"]),
        ..old
    });
    fs_operation! { SetManipulation, sim, "myset",
        file "placeholder" "x"
    };

    sim.dryrun(true);
    let result = monja::add(
        &sim.profile()?,
        sim.execution_options(),
        SetName("myset".into()),
        vec!["git".into()],
    )?;

    expect_that!(result.added, unordered_elements_are![eq("git")]);

    let config = SetConfig::load(&sim.profile()?, &SetName("myset".into()))?;
    expect_that!(config.packages, is_empty());

    Ok(())
}

#[gtest]
fn remove_present_and_not_present() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["myset"]),
        ..old
    });
    fs_operation! { SetManipulation, sim, "myset",
        file "placeholder" "x"
    };
    sim.configure_set(SetName("myset".into()), |old| SetConfig {
        packages: vec!["git".into(), "neovim".into()],
        ..old
    });

    let result = monja::remove(
        &sim.profile()?,
        sim.execution_options(),
        SetName("myset".into()),
        vec!["git".into(), "doesnotexist".into()],
    )?;

    expect_that!(result.removed, unordered_elements_are![eq("git")]);
    expect_that!(
        result.not_present,
        unordered_elements_are![eq("doesnotexist")]
    );

    let config = SetConfig::load(&sim.profile()?, &SetName("myset".into()))?;
    expect_that!(config.packages, unordered_elements_are![eq("neovim")]);

    Ok(())
}

#[gtest]
fn remove_dry_run() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["myset"]),
        ..old
    });
    fs_operation! { SetManipulation, sim, "myset",
        file "placeholder" "x"
    };
    sim.configure_set(SetName("myset".into()), |old| SetConfig {
        packages: vec!["git".into()],
        ..old
    });

    sim.dryrun(true);
    let result = monja::remove(
        &sim.profile()?,
        sim.execution_options(),
        SetName("myset".into()),
        vec!["git".into()],
    )?;

    expect_that!(result.removed, unordered_elements_are![eq("git")]);

    let config = SetConfig::load(&sim.profile()?, &SetName("myset".into()))?;
    expect_that!(config.packages, unordered_elements_are![eq("git")]);

    Ok(())
}

#[gtest]
fn remove_set_not_found() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["myset"]),
        ..old
    });

    let result = monja::remove(
        &sim.profile()?,
        sim.execution_options(),
        SetName("nonexistent".into()),
        vec!["git".into()],
    );

    expect_that!(result, err(anything()));

    Ok(())
}

#[gtest]
fn list_merges_across_sets() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });
    fs_operation! { SetManipulation, sim, "set1", file "placeholder" "x" };
    fs_operation! { SetManipulation, sim, "set2", file "placeholder" "x" };
    sim.configure_set(SetName("set1".into()), |old| SetConfig {
        packages: vec!["git".into(), "neovim".into()],
        ..old
    });
    sim.configure_set(SetName("set2".into()), |old| SetConfig {
        packages: vec!["neovim".into(), "ripgrep".into()],
        ..old
    });

    let result = monja::list(&sim.profile()?)?;

    expect_that!(
        result.by_set,
        elements_are![
            (
                pat!(SetName("set1")),
                unordered_elements_are![eq("git"), eq("neovim")]
            ),
            (
                pat!(SetName("set2")),
                unordered_elements_are![eq("neovim"), eq("ripgrep")]
            ),
        ]
    );
    expect_that!(
        result.merged,
        elements_are![eq("git"), eq("neovim"), eq("ripgrep")]
    );

    Ok(())
}

#[gtest]
fn list_errors_on_missing_target_set() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });
    fs_operation! { SetManipulation, sim, "set1", file "placeholder" "x" };
    sim.configure_set(SetName("set1".into()), |old| SetConfig {
        packages: vec!["git".into()],
        ..old
    });
    // set2 is never created in the repo

    let result = monja::list(&sim.profile()?);

    expect_that!(
        result,
        err(pat!(monja::ListError::MissingSets(elements_are![pat!(
            SetName("set2")
        )])))
    );

    Ok(())
}

#[gtest]
fn install_returns_merged_list() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    });
    fs_operation! { SetManipulation, sim, "set1", file "placeholder" "x" };
    sim.configure_set(SetName("set1".into()), |old| SetConfig {
        packages: vec!["git".into(), "neovim".into()],
        ..old
    });

    let result = monja::install(&sim.profile()?, sim.execution_options())?;
    expect_that!(result.packages, elements_are![eq("git"), eq("neovim")]);

    Ok(())
}

#[gtest]
fn install_errors_on_missing_target_set() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1", "set2"]),
        ..old
    });
    fs_operation! { SetManipulation, sim, "set1", file "placeholder" "x" };
    sim.configure_set(SetName("set1".into()), |old| SetConfig {
        packages: vec!["git".into()],
        ..old
    });
    // set2 is never created in the repo

    let result = monja::install(&sim.profile()?, sim.execution_options());

    expect_that!(
        result,
        err(pat!(monja::InstallError::MissingSets(elements_are![pat!(
            SetName("set2")
        )])))
    );

    Ok(())
}

#[gtest]
fn install_dry_run() -> Result<()> {
    let mut sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    });
    fs_operation! { SetManipulation, sim, "set1", file "placeholder" "x" };
    sim.configure_set(SetName("set1".into()), |old| SetConfig {
        packages: vec!["git".into()],
        ..old
    });

    sim.dryrun(true);
    let result = monja::install(&sim.profile()?, sim.execution_options())?;
    expect_that!(result.packages, elements_are![eq("git")]);

    Ok(())
}

#[gtest]
fn install_no_command_configured() -> Result<()> {
    let sim = Simulator::create();
    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        ..old
    });
    fs_operation! { SetManipulation, sim, "set1", file "placeholder" "x" };
    sim.configure_set(SetName("set1".into()), |old| SetConfig {
        packages: vec!["git".into()],
        ..old
    });

    let result = monja::install(&sim.profile()?, sim.execution_options())?;

    expect_that!(result.command, none());
    expect_that!(result.executed, is_false());

    Ok(())
}

#[gtest]
fn install_does_not_run_configured_command_when_no_packages() -> Result<()> {
    let sim = Simulator::create();
    let output_path = sim.data_root().join("installed.txt");

    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        packages: monja::packages::Config {
            install_command: Some(format!("echo {{packages}} > {}", output_path.display())),
            ..Default::default()
        },
        ..old
    });
    // set1 has no packages declared
    fs_operation! { SetManipulation, sim, "set1", file "placeholder" "x" };

    let result = monja::install(&sim.profile()?, sim.execution_options())?;

    expect_that!(result.packages, is_empty());
    expect_that!(result.command, none());
    expect_that!(result.executed, is_false());
    expect_that!(output_path.exists(), is_false());

    Ok(())
}

#[gtest]
fn install_runs_configured_command_for_single_package() -> Result<()> {
    let sim = Simulator::create();
    let output_path = sim.data_root().join("installed.txt");

    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        packages: monja::packages::Config {
            install_command: Some(format!("echo {{packages}} > {}", output_path.display())),
            ..Default::default()
        },
        ..old
    });
    fs_operation! { SetManipulation, sim, "set1", file "placeholder" "x" };
    sim.configure_set(SetName("set1".into()), |old| SetConfig {
        packages: vec!["git".into()],
        ..old
    });

    let result = monja::install(&sim.profile()?, sim.execution_options())?;

    expect_that!(result.executed, is_true());
    expect_that!(
        result.command,
        some(eq(&format!("echo git > {}", output_path.display())))
    );

    let written = std::fs::read_to_string(&output_path)?;
    expect_that!(written.trim(), eq("git"));

    Ok(())
}

#[gtest]
fn install_uses_default_delimiter_when_unconfigured() -> Result<()> {
    let sim = Simulator::create();
    let output_path = sim.data_root().join("installed.txt");

    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        packages: monja::packages::Config {
            install_command: Some(format!("echo {{packages}} > {}", output_path.display())),
            ..Default::default()
        },
        ..old
    });
    fs_operation! { SetManipulation, sim, "set1", file "placeholder" "x" };
    sim.configure_set(SetName("set1".into()), |old| SetConfig {
        packages: vec!["git".into(), "neovim".into()],
        ..old
    });

    let result = monja::install(&sim.profile()?, sim.execution_options())?;

    expect_that!(result.executed, is_true());
    expect_that!(
        result.command,
        some(eq(&format!("echo git neovim > {}", output_path.display())))
    );

    let written = std::fs::read_to_string(&output_path)?;
    expect_that!(written.trim(), eq("git neovim"));

    Ok(())
}

#[gtest]
fn install_runs_configured_command_with_aliases_and_delimiter() -> Result<()> {
    let sim = Simulator::create();
    let output_path = sim.data_root().join("installed.txt");

    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        packages: monja::packages::Config {
            install_command: Some(format!("echo {{packages}} > {}", output_path.display())),
            install_delimiter: Some(",".into()),
            aliases: std::collections::HashMap::from([(
                "neovim".to_string(),
                "neovim-git".to_string(),
            )]),
        },
        ..old
    });
    fs_operation! { SetManipulation, sim, "set1", file "placeholder" "x" };
    sim.configure_set(SetName("set1".into()), |old| SetConfig {
        packages: vec!["git".into(), "neovim".into()],
        ..old
    });

    let result = monja::install(&sim.profile()?, sim.execution_options())?;

    expect_that!(result.executed, is_true());
    expect_that!(
        result.command,
        some(eq(&format!(
            "echo git,neovim-git > {}",
            output_path.display()
        )))
    );

    let written = std::fs::read_to_string(&output_path)?;
    expect_that!(written.trim(), eq("git,neovim-git"));

    Ok(())
}

#[gtest]
fn install_dry_run_does_not_execute_configured_command() -> Result<()> {
    let mut sim = Simulator::create();
    let output_path = sim.data_root().join("installed.txt");

    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        packages: monja::packages::Config {
            install_command: Some(format!("echo {{packages}} > {}", output_path.display())),
            ..Default::default()
        },
        ..old
    });
    fs_operation! { SetManipulation, sim, "set1", file "placeholder" "x" };
    sim.configure_set(SetName("set1".into()), |old| SetConfig {
        packages: vec!["git".into()],
        ..old
    });

    sim.dryrun(true);
    let result = monja::install(&sim.profile()?, sim.execution_options())?;

    expect_that!(result.executed, is_false());
    expect_that!(result.command, some(anything()));
    expect_that!(output_path.exists(), is_false());

    Ok(())
}

#[gtest]
fn install_command_failure() -> Result<()> {
    let sim = Simulator::create();

    sim.configure_profile(|old| MonjaProfileConfig {
        target_sets: set_names(["set1"]),
        packages: monja::packages::Config {
            install_command: Some("exit 1".into()),
            ..Default::default()
        },
        ..old
    });
    fs_operation! { SetManipulation, sim, "set1", file "placeholder" "x" };
    sim.configure_set(SetName("set1".into()), |old| SetConfig {
        packages: vec!["git".into()],
        ..old
    });

    let result = monja::install(&sim.profile()?, sim.execution_options());

    expect_that!(result, err(pat!(monja::InstallError::CommandFailed(_))));

    Ok(())
}
