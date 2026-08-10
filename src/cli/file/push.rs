use clap::Args;
use monja::{ExecutionOptions, MonjaProfile};

#[derive(Args)]
pub struct PushCommand {}
impl PushCommand {
    pub fn execute(&self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        let result = monja::push(&profile, &opts);

        // want better logging for this
        if let Err(monja::PushError::Consistency {
            files_with_missing_sets,
            missing_files,
        }) = result
        {
            let mut print_generic = false;
            if !files_with_missing_sets.is_empty() {
                print_generic = true;

                eprintln!("There are local files whose corresponding sets are missing.");

                eprintln!("Sets missing, as well as the files that currently require them:");
                for (set_name, file_paths) in files_with_missing_sets {
                    eprintln!("\tSet: {}", set_name);
                    for path in file_paths {
                        eprintln!("\t\t{}", path);
                    }
                }
            }
            if !missing_files.is_empty() {
                print_generic = true;

                eprintln!("There are local files missing from expected sets.");

                eprintln!("Files missing, as grouped under the sets they were expected to be in:");
                for (set_name, file_paths) in missing_files {
                    eprintln!("\tSet: {}", set_name);
                    for path in file_paths {
                        eprintln!("\t\t{}", path);
                    }
                }
            }

            if print_generic {
                eprint!(
                    "This happens due to changes being made in the repo without having yet pulled."
                );
                eprint!(
                    "It is recommended to `monja file push` before doing a `git pull` or other repo modification."
                );
                eprintln!("To fix this, consider doing any of the the following:");

                eprintln!(
                    "\t* If there are no local changes that would get overwritten, use `monja file pull`."
                );

                eprint!(
                    "\t* If the files should use a different set (such as the last specified in monja-profile.toml), "
                );
                eprint!(
                    "use some variation of `monja file put` to specify that set and copy files to that set. "
                );
                eprintln!(
                    "Then, use `monja file push` to push the rest of the files to the right set."
                );

                eprint!("\t* If the file is no longer needed, simply delete it. ");
                eprintln!(
                    "Then, use `monja file push` to push these and the rest of the files to the right set."
                );

                eprintln!("\t* Manually merge local changes into the repo, then `monja file pull`.");
            }

            // probably something better to use, but we don't want to double log with the below `result?`.
            return Err(anyhow::Error::msg("Failed to push."));
        }

        // log rest of errors like this because lazy
        let result = result?;

        if !result.files_pushed.is_empty() {
            println!(
                "Files pushed (including unchanged), as grouped under their corresponding sets:"
            );
            for (set_name, file_paths) in result.files_pushed.iter() {
                eprintln!("\tSet: {}", set_name);
                for path in file_paths {
                    eprintln!("\t\t{}", path);
                }
            }
        } else {
            println!("No files pushed.");
        }

        Ok(())
    }
}
