use clap::Args;
use monja::{ExecutionOptions, MonjaProfile};

#[derive(Args)]
pub struct RepoDirCommand {}
impl RepoDirCommand {
    pub fn execute(&self, profile: MonjaProfile, _opts: ExecutionOptions) -> anyhow::Result<()> {
        println!("{}", profile.repo_root);

        Ok(())
    }
}
