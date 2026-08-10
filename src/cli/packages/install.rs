use clap::Args;
use monja::{ExecutionOptions, MonjaProfile};

#[derive(Args)]
pub struct InstallCommand {}

impl InstallCommand {
    pub fn execute(&self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        let result = monja::install(&profile, &opts);

        if let Err(monja::InstallError::MissingSets(missing_sets)) = result {
            eprintln!(
                "Sets needed by the profile are missing from the repo: {:?}",
                missing_sets
            );
            eprintln!("Verify that the right set of sets in 'monja-profile.toml' are present.");
            return Err(anyhow::Error::msg("Failed to install packages."));
        }

        let result = result?;

        if !result.packages.is_empty() {
            println!(
                "The following packages would be installed (installation is not yet implemented):"
            );
            for package in result.packages.iter() {
                println!("\t{}", package);
            }
        } else {
            println!("No packages to install.");
        }

        Ok(())
    }
}
