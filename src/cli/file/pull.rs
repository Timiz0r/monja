use clap::Args;
use monja::{ExecutionOptions, MonjaProfile};

#[derive(Args)]
pub struct PullCommand {}
impl PullCommand {
    pub fn execute(&self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        let result = monja::pull(&profile, &opts);

        if let Err(monja::PullError::MissingSets(missing_sets)) = result {
            eprintln!(
                "Sets needed by the profile are missing from the repo: {:?}",
                missing_sets
            );
            eprintln!("Verify that the right set of sets in 'monja-profile.toml' are present.");
            // probably something better to use, but we don't want to double log with the below `result?`.
            return Err(anyhow::Error::msg("Failed to pull."));
        }

        let result = result?;

        if !result.files_pulled.is_empty() {
            println!(
                "Files pulled (including unchanged), as grouped under their corresponding sets:"
            );
            for (set_name, file_paths) in result.files_pulled.into_iter() {
                println!("\tSet: {}", set_name);
                for path in file_paths {
                    println!(
                        "\t\t'{}' -> '{}'",
                        path.path_in_set.display(),
                        path.local_path.display()
                    );
                }
            }
        } else {
            println!("No files pulled.");
        }

        if !result.cleanable_files.is_empty() {
            println!("There are files present locally that are no longer pulled from the repo.");
            println!("If this is expected, do a `monja file clean` to remove them.");
            println!(
                "If any are unexpected, copy them to a new set before performing `monja file clean`."
            );

            for file_path in result.cleanable_files.into_iter() {
                println!("\t{}", file_path);
            }
        }

        Ok(())
    }
}
