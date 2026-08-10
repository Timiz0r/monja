use clap::Args;
use monja::{ExecutionOptions, MonjaProfile};

#[derive(Args)]
pub struct ListCommand {}

impl ListCommand {
    pub fn execute(&self, profile: MonjaProfile, _opts: ExecutionOptions) -> anyhow::Result<()> {
        let result = monja::list(&profile);

        if let Err(monja::ListError::MissingSets(missing_sets)) = result {
            eprintln!(
                "Sets needed by the profile are missing from the repo: {:?}",
                missing_sets
            );
            eprintln!("Verify that the right set of sets in 'monja-profile.toml' are present.");
            return Err(anyhow::Error::msg("Failed to list packages."));
        }

        let result = result?;

        if !result.by_set.is_empty() {
            println!("Packages, as grouped under their corresponding sets:");
            for (set_name, packages) in result.by_set.iter() {
                println!("\tSet: {}", set_name);
                for package in packages {
                    println!("\t\t{}", package);
                }
            }
        } else {
            println!("No targeted sets declare any packages.");
        }

        println!();
        if !result.merged.is_empty() {
            println!("Merged (effective) package list for this profile:");
            for package in result.merged.iter() {
                println!("\t{}", package);
            }
        } else {
            println!("No packages in the merged (effective) list.");
        }

        Ok(())
    }
}
