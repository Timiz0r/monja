use std::collections::HashSet;

use crate::{MonjaProfile, set};

pub mod add;
pub mod install;
pub mod list;
pub mod remove;

pub(crate) mod repo;

// unlike files, a package has no "content" to conflict over -- it's just a name that either
// is or isn't wanted -- so merging targeted sets' packages is a plain deduplicated union,
// with no need for files::repo::RepoState::get_owning_set-style last-set-wins resolution.
pub(crate) struct PackageSets {
    pub by_set: Vec<(set::SetName, Vec<String>)>,
    pub merged: Vec<String>,
}

// targeted sets missing from the repo are a hard error, same as `pull`: a partial package list
// (from `list`) or a partial install (from `install`) is misleading state to act on silently, so
// we'd rather surface a misconfigured/incomplete repo checkout early than guess.
pub(crate) fn gather(
    profile: &MonjaProfile,
    repo: &repo::RepoState,
) -> Result<PackageSets, Vec<set::SetName>> {
    let mut by_set = Vec::with_capacity(profile.config.target_sets.len());
    let mut seen = HashSet::new();
    let mut merged = Vec::new();
    let mut missing_sets = Vec::new();

    for set_name in profile.config.target_sets.iter() {
        let Some(set) = repo.sets.get(set_name) else {
            missing_sets.push(set_name.clone());
            continue;
        };

        // if we find a missing set, save us the trouble of handling the rest
        if !missing_sets.is_empty() {
            continue;
        }

        for package in set.packages.iter() {
            if seen.insert(package.clone()) {
                merged.push(package.clone());
            }
        }

        by_set.push((set_name.clone(), set.packages.clone()));
    }

    if !missing_sets.is_empty() {
        return Err(missing_sets);
    }

    Ok(PackageSets { by_set, merged })
}
