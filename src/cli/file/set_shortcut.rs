use std::path::PathBuf;

use anyhow::anyhow;
use clap::Args;
use clap_complete::engine::ArgValueCandidates;
use monja::{AbsolutePath, ExecutionOptions, MonjaProfile, SetName};

use crate::completions;

#[derive(Args)]
pub struct SetShortcutCommand {
    /// The set whose shortcut to change
    #[arg(long = "set", add = ArgValueCandidates::new(completions::set_names))]
    set_name: String,

    /// The new shortcut path (relative to local root)
    path: PathBuf,
}
impl SetShortcutCommand {
    pub fn execute(self, profile: MonjaProfile, opts: ExecutionOptions) -> anyhow::Result<()> {
        let path = if self.path.is_absolute() {
            self.path
        } else {
            let cwd = AbsolutePath::for_existing_path(&std::env::current_dir()?)?;
            cwd.join(&self.path)
        };
        let path = path
            .strip_prefix(&profile.local_root)
            .map_err(|_| {
                anyhow!(
                    "Path '{}' is not under local root '{}'",
                    path.display(),
                    profile.local_root
                )
            })?
            .to_path_buf();

        let set_name = SetName(self.set_name);

        let result = monja::set_shortcut(&profile, &opts, set_name, path)?;

        if result.old_shortcut.as_os_str().is_empty() {
            println!(
                "Set `{}` shortcut changed from (none) to '{}'.",
                result.set_name,
                result.new_shortcut.display()
            );
        } else if result.new_shortcut.as_os_str().is_empty() {
            println!(
                "Set `{}` shortcut changed from '{}' to (none).",
                result.set_name,
                result.old_shortcut.display()
            );
        } else {
            println!(
                "Set `{}` shortcut changed from '{}' to '{}'.",
                result.set_name,
                result.old_shortcut.display(),
                result.new_shortcut.display()
            );
        }
        for file in result.files_moved.iter() {
            println!("\t{}", file.display());
        }
        println!("{} file(s) restructured.", result.files_moved.len());

        Ok(())
    }
}
