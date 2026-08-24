use clap::Args;
use clap_complete::engine::ArgValueCandidates;
use monja::{ExecutionOptions, MonjaProfile, RepoName};

use crate::completions;

#[derive(Args)]
pub struct RepoDirCommand {
    /// The repo to print.
    ///
    /// Only needed when the profile has several repos and no `default-repo` is configured.
    #[arg(long = "repo", add = ArgValueCandidates::new(completions::repo_names))]
    repo: Option<String>,
}
impl RepoDirCommand {
    pub fn execute(&self, profile: MonjaProfile, _opts: ExecutionOptions) -> anyhow::Result<()> {
        let repo = self.repo.as_deref().map(RepoName::from);
        let (_, root) = profile.resolve_repo(repo.as_ref())?;

        println!("{}", root);

        Ok(())
    }
}
