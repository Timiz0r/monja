use clap::Args;
use clap_complete::engine::ArgValueCandidates;
use monja::{ExecutionOptions, MonjaProfile, SetName};

use crate::completions;

#[derive(Args)]
pub struct AddCommand {
    /// The set to add the packages to
    #[arg(long = "set", add = ArgValueCandidates::new(completions::set_names))]
    set_name: String,

    /// The package names to add
    packages: Vec<String>,
}

impl AddCommand {
    pub fn execute(self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        if self.packages.is_empty() {
            println!("No packages specified.");
            return Ok(());
        }

        let result = monja::add(&profile, &opts, SetName(self.set_name), self.packages)?;

        if !result.added.is_empty() {
            println!(
                "Added the following packages to set `{}` (in repo `{}`):",
                result.set_name, result.repo
            );
            for package in result.added.iter() {
                println!("\t{}", package);
            }
        }

        if !result.already_present.is_empty() {
            println!(
                "The following packages were already in set `{}`:",
                result.set_name
            );
            for package in result.already_present.iter() {
                println!("\t{}", package);
            }
        }

        Ok(())
    }
}
