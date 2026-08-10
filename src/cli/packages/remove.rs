use clap::Args;
use clap_complete::engine::ArgValueCandidates;
use monja::{ExecutionOptions, MonjaProfile, SetName};

use crate::completions;

#[derive(Args)]
pub struct RemoveCommand {
    /// The set to remove the packages from
    #[arg(long = "set", add = ArgValueCandidates::new(completions::set_names))]
    set_name: String,

    /// The package names to remove
    packages: Vec<String>,
}

impl RemoveCommand {
    pub fn execute(self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        if self.packages.is_empty() {
            println!("No packages specified.");
            return Ok(());
        }

        let result = monja::remove(&profile, &opts, SetName(self.set_name), self.packages)?;

        if !result.removed.is_empty() {
            println!(
                "Removed the following packages from set `{}`:",
                result.set_name
            );
            for package in result.removed.iter() {
                println!("\t{}", package);
            }
        }

        if !result.not_present.is_empty() {
            println!(
                "The following packages weren't in set `{}`:",
                result.set_name
            );
            for package in result.not_present.iter() {
                println!("\t{}", package);
            }
        }

        Ok(())
    }
}
