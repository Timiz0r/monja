use clap::Args;
use monja::{CleanMode, ExecutionOptions, MonjaProfile};

#[derive(Args)]
pub struct CleanCommand {
    /// If set, compares the full state of the repo against the local state,
    /// cleaning files that are not tracked in the repo.
    /// If not set, the previous two `monja file pull`s are used to determine which files to clean.
    #[arg(long, short)]
    full: bool,
}
impl CleanCommand {
    pub fn execute(&self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        let mode = match self.full {
            true => CleanMode::Full,
            false => CleanMode::Index,
        };
        let clean_result = monja::clean(&profile, &opts, mode)?;

        if !clean_result.files_cleaned.is_empty() {
            println!("Local files cleaned:");
            for path in clean_result.files_cleaned.into_iter() {
                println!("{}", path);
            }
        } else {
            println!("No local files cleaned.")
        }

        Ok(())
    }
}
