use std::process::Command;

use thiserror::Error;

use crate::{ExecutionOptions, MonjaProfile, set};

use super::{Config, PackageSets, repo};

#[derive(Error, Debug)]
pub enum InstallError {
    #[error("Unable to initialize repo state:{}", crate::format_errors(.0))]
    RepoStateInitialization(Vec<repo::StateInitializationError>),

    #[error("Sets needed by the profile are missing from the repo.")]
    MissingSets(Vec<set::SetName>),

    #[error("Install cancelled by user.")]
    UserCancellation,

    #[error("Failed to run install command.")]
    CommandFailed(#[source] std::io::Error),
}

#[derive(Debug)]
pub struct InstallSuccess {
    // the merged, canonical (monja) package names -- not alias-translated.
    pub packages: Vec<String>,

    // the fully-formatted command (aliases applied, joined by the delimiter, substituted into
    // the template) -- None if the profile doesn't configure `packages.install-command`.
    pub command: Option<String>,

    // false on dry-run, when there's nothing to install, or when no install command is
    // configured -- true only once the command has actually been run successfully.
    pub executed: bool,
}

pub fn install(
    profile: &MonjaProfile,
    opts: &ExecutionOptions,
) -> Result<InstallSuccess, InstallError> {
    let repo_state =
        repo::initialize_state(profile).map_err(InstallError::RepoStateInitialization)?;

    let PackageSets { merged, .. } =
        super::gather(profile, &repo_state).map_err(InstallError::MissingSets)?;

    if merged.is_empty() {
        return Ok(InstallSuccess {
            packages: merged,
            command: None,
            executed: false,
        });
    }

    let command = build_command(&profile.config.packages, &merged);

    let executed = match &command {
        Some(command) if !opts.dry_run => {
            if !opts.user_confirm(&format!("About to run:\n\t{}", command)) {
                return Err(InstallError::UserCancellation);
            }
            run(command)?;
            true
        }
        _ => false,
    };

    Ok(InstallSuccess {
        packages: merged,
        command,
        executed,
    })
}

// applies aliases (falling back to the canonical name when a package has none), joins with the
// configured delimiter (default: a single space), and substitutes the result into the `{packages}`
// token of the configured template. returns None if no `install-command` is configured.
fn build_command(config: &Config, packages: &[String]) -> Option<String> {
    let template = config.install_command.as_ref()?;
    let delimiter = config.install_delimiter.as_deref().unwrap_or(" ");

    let aliased: Vec<&str> = packages
        .iter()
        .map(|p| config.aliases.get(p).map_or(p.as_str(), String::as_str))
        .collect();
    let joined = aliased.join(delimiter);

    Some(template.replace("{packages}", &joined))
}

// runs the command through `sh -c` (so the configured string can use arbitrary shell syntax --
// quoting, `&&`, `sudo`, etc.) with stdio inherited rather than captured, so an interactive
// prompt (e.g. a `sudo` password prompt) reaches the user's terminal directly.
fn run(command: &str) -> Result<(), InstallError> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(command)
        .status()
        .map_err(InstallError::CommandFailed)?;

    if status.success() {
        Ok(())
    } else {
        Err(InstallError::CommandFailed(std::io::Error::other(format!(
            "Install command exited with status {:?}",
            status.code()
        ))))
    }
}
