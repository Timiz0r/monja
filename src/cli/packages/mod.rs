use clap::{Args, Subcommand};
use monja::{ExecutionOptions, MonjaProfile};

mod add;
mod install;
mod list;
mod remove;

use add::AddCommand;
use install::InstallCommand;
use list::ListCommand;
use remove::RemoveCommand;

#[derive(Args)]
pub struct PackageArgs {
    #[command(subcommand)]
    command: PackageCommands,
}
impl PackageArgs {
    pub fn execute(self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        self.command.execute(profile, opts)
    }
}

#[derive(Subcommand)]
#[command(rename_all = "lower")]
enum PackageCommands {
    /// Adds packages to a set.
    ///
    /// Unlike files, packages have no local reflection to sync -- they're just names declared
    /// directly in the set's `.monja-set.toml`.
    Add(AddCommand),

    /// Removes packages from a set.
    Remove(RemoveCommand),

    /// Lists packages declared by each targeted set, as well as the merged (effective) list.
    ///
    /// Unlike files, merging packages across targeted sets is a simple deduplicated union --
    /// there's no per-package content to have the latest set win over.
    List(ListCommand),

    /// Installs the merged (effective) package list for this profile.
    ///
    /// Installation isn't implemented yet -- this currently only reports what would be installed.
    Install(InstallCommand),
}
impl PackageCommands {
    fn execute(self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        match self {
            PackageCommands::Add(command) => command.execute(profile, opts),
            PackageCommands::Remove(command) => command.execute(profile, opts),
            PackageCommands::List(command) => command.execute(profile, opts),
            PackageCommands::Install(command) => command.execute(profile, opts),
        }
    }
}
