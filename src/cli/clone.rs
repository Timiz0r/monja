use std::path::PathBuf;

use clap::Args;
use monja::{AbsolutePath, CloneSpec, ExecutionOptions, RepoName};

#[derive(Args)]
pub struct CloneCommand {
    /// The name to give the repo in the profile.
    #[arg(long = "repo")]
    repo: String,

    /// The git URL to clone from.
    url: String,
}

impl CloneCommand {
    pub fn execute(
        self,
        opts: ExecutionOptions,
        profile_config_path: PathBuf,
        local_root: AbsolutePath,
        data_root: AbsolutePath,
    ) -> anyhow::Result<()> {
        let spec = CloneSpec {
            profile_config_path,
            local_root,
            data_root,
            repo_name: RepoName::from(self.repo.as_str()),
            url: self.url,
        };
        let result = monja::clone(&opts, spec)?;

        if result.profile.is_none() {
            println!("No changes made because dry-run.");
            return Ok(());
        }

        println!(
            "Cloned repo '{}' into '{}'.",
            result.repo_name,
            result.repo_root.display()
        );
        match result.profile_created {
            true => println!(
                "A profile was created at '{}'.",
                result.profile_config_path.display()
            ),
            false => println!(
                "The repo was added to the profile at '{}'.",
                result.profile_config_path.display()
            ),
        };
        println!(
            "No sets are targeted yet. Add the ones you want to 'target-sets', then run `monja file pull`."
        );

        Ok(())
    }
}
