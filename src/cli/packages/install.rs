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

        if result.packages.is_empty() {
            println!("No packages to install.");
            return Ok(());
        }

        println!("Packages (merged, effective list for this profile):");
        for package in result.packages.iter() {
            println!("\t{}", package);
        }

        match result.command {
            Some(command) if result.executed => println!("\nRan: {}", command),
            Some(command) => println!("\nWould run: {}", command),
            None => println!(
                "\nNo install command configured for this profile \
                 (set `packages.install-command` in monja-profile.toml)."
            ),
        }

        Ok(())
    }
}
