use std::{fs, path::PathBuf};

use clap::Args;
use monja::{AbsolutePath, ExecutionOptions, InitOutcome, InitSpec, RepoName};

#[derive(Args)]
pub struct InitCommand {
    /// The name to give the repo.
    ///
    /// When monja is already initialized, this creates an additional, empty repo and adds it to
    /// the profile. To bring in a repo that already exists elsewhere, use `monja clone` instead.
    #[arg(long = "repo")]
    repo: Option<String>,
}
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
            repo_name: self.repo.as_deref().map(RepoName::from),
            initial_set_name: machine,
        };
        let result = monja::init(&opts, spec)?;

        let Some(profile) = result.profile else {
            println!("No changed made because dry-run.");
            return Ok(());
        };

        match result.outcome {
            InitOutcome::Initialized { initial_set } => {
                println!("Initialization successful!");
                println!(
                    "Profile can be found at '{}'.",
                    result.profile_config_path.display()
                );
                for (name, root) in profile.repos.iter() {
                    println!("Repo '{}' can be found in '{}'.", name, root);
                }
                println!("Set '{}' automatically created.", initial_set);
            }
            InitOutcome::RepoAdded => {
                println!(
                    "Repo '{}' created in '{}' and added to the profile at '{}'.",
                    result.repo_name,
                    result.repo_root.display(),
                    result.profile_config_path.display()
                );
                println!(
                    "It's empty, and no sets are targeted from it. Use `monja newset --repo {}` to add one.",
                    result.repo_name
                );
            }
        };

        Ok(())
    }
}
