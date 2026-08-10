use clap::Args;
use monja::{ExecutionOptions, MonjaProfile};

#[derive(Args)]
pub struct ProfileCommand {}
impl ProfileCommand {
    pub fn execute(&self, _profile: MonjaProfile, _opts: ExecutionOptions) -> anyhow::Result<()> {
        // TODO: dedupe logic. used here, in main, and in NewSetCommand
        let base = xdg::BaseDirectories::with_prefix("monja");
        let path = base.place_config_file("monja-profile.toml")?;

        println!("{}", path.display());

        Ok(())
    }
}
