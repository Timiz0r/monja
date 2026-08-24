use std::{fs, path::PathBuf};

use clap::Args;
use monja::{AbsolutePath, ExecutionOptions, InitSpec, RepoName};

#[derive(Args)]
pub struct InitCommand {}
impl InitCommand {
    pub fn execute(
        &self,
        opts: ExecutionOptions,
        profile_config_path: PathBuf,
        local_root: AbsolutePath,
        data_root: AbsolutePath,
    ) -> anyhow::Result<()> {
        let machine = fs::read_to_string("/proc/sys/kernel/hostname")
            .expect("If doesn't exist, would prefer panic.")
            .trim()
            .to_string();

        let spec = InitSpec {
            profile_config_path,
            local_root,
            data_root,
            repo_name: RepoName::default_name(),
            initial_set_name: machine,
        };
        let result = monja::init(&opts, spec)?;

        match result.profile {
            Some(profile) => {
                println!("Initialization successful!");
                println!(
                    "Profile can be found at '{}'.",
                    result.profile_config_path.display()
                );
                for (name, root) in profile.repos.iter() {
                    println!("Repo '{}' can be found in '{}'.", name, root);
                }
                println!(
                    "Set '{}' automatically created.",
                    profile.config.target_sets[0]
                );
            }
            None => println!("No changed made because dry-run."),
        };

        Ok(())
    }
}
