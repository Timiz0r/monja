use std::{collections::HashMap, path::PathBuf};

use crate::files;

#[derive(PartialEq, Eq, Hash)]
pub struct SetName(String);

pub struct Monja {
    repo_root: PathBuf,
    sets: Vec<SetName>,
    default_set: HashMap<PathBuf, SetName>,
}

impl Monja {
    // TODO: return result and less unwraps
    pub fn push(&self) {
        let mut repo = files::repo::initialize_full_state(&self.repo_root);

        let mut sets = repo.select_sets_mut(self.sets.iter());
        for set in sets.iter_mut() {
            for local_file in files::local::walk(set.root()) {
                let local_file = local_file.unwrap();
                if let Some(repo_file) = set.file_from_local(&local_file) {
                    // plus zero or more found dirs
                    repo_file.mark_found();
                    repo_file.update_permissions(local_file.get_permissions());
                } else {
                    // plus zero or more stub dirs
                    set.mark_new_file(&local_file);
                }
            }
        }

        // TODO: log a summary and ask for confirmation. and dryrun. and whatnot

        for set in sets.iter() {
            // writing files can modify monja-dir.toml files, so we do dirs first to ensure they exist
            for dir in set.dirs() {
                if !dir.found_locally() {
                    dir.delete();
                    continue;
                }

                // while doing the rsync part first is another way to ensure directories exist,
                // one intended consequence of .monja-dir files is that they ensure empty dirs exist.
                // rsync won't do that, not that it's that important of a detail.
                dir.write_dir_config();
            }
        }

        // we use rsync to avoid reinventing the wheel of high-perf copying
        for set in sets.iter() {
            for dir in set.dirs() {
                rsync(dir);
            }
        }
    }
}

fn rsync(dest: &files::repo::Directory) {
    todo!()
}
