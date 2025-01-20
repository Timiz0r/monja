use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use exacl::AclEntry;

use crate::monja::SetName;

pub(crate) struct State {
    sets: HashMap<SetName, Set>,
}
impl State {
    pub(crate) fn select_sets_mut<'a, T>(&mut self, sets: T) -> Vec<&mut Set>
    where
        T: Iterator<Item = &'a SetName>,
    {
        let sets: HashSet<&SetName> = HashSet::from_iter(sets);
        self.sets
            .iter_mut()
            .filter(|(name, _)| sets.contains(name))
            .map(|(_, set)| set)
            .collect::<Vec<&mut Set>>()
        // self.sets.iter_mut().map(|)
    }
}

pub(crate) struct Set {
    root: PathBuf,
    directories: HashMap<PathBuf, Directory>,
    files: HashMap<PathBuf, File>,
    locally_mapped_files: HashMap<PathBuf, File>,
    nopush: bool,
    // TODO: packages
}
impl Set {
    pub(crate) fn file_from_local(&mut self, file: &super::local::File) -> Option<&mut File> {
        self.locally_mapped_files.get_mut(file.path())
    }

    pub(crate) fn mark_new_file(&mut self, local_file: &super::local::File) {
        todo!()
    }

    pub(crate) fn dirs_mut(&mut self) -> impl Iterator<Item = &mut Directory> {
        self.directories.values_mut()
    }

    pub(crate) fn dirs(&self) -> impl Iterator<Item = &Directory> {
        self.directories.values()
    }

    pub(crate) fn files(&self) -> impl Iterator<Item = &File> {
        self.files.values()
    }

    pub(crate) fn root(&self) -> &std::path::Path {
        &self.root
    }
}

pub(crate) struct Directory {
    repo_path: PathBuf,
    files: Vec<File>,
    permissions: Vec<exacl::AclEntry>,
    // default_permissions: Vec<exacl::AclEntry>,
    noclean: bool,

    found_locally: bool,
}
impl Directory {
    pub(crate) fn found_locally(&self) -> bool {
        self.found_locally
    }

    pub(crate) fn delete(&self) -> std::io::Result<()> {
        std::fs::remove_dir_all(&self.repo_path)
    }

    pub(crate) fn write_dir_config(&self) -> std::io::Result<()> {
        todo!()
    }
}

pub(crate) struct File {
    repo_path: PathBuf,
    local_path: PathBuf,
    permissions: Vec<exacl::AclEntry>,

    found_locally: bool,
}
impl File {
    pub(crate) fn mark_found(&mut self) {
        self.found_locally = true;
    }

    pub(crate) fn update_permissions(&mut self, perms: Vec<AclEntry>) {
        self.permissions = perms;
    }

    fn delete(&self) -> std::io::Result<()> {
        std::fs::remove_file(&self.repo_path)
    }
}

// could do lazy initialization, but prefer the very explicit nature of having multiple initialization functions
pub(crate) fn initialize_full_state(root: &Path) -> State {
    todo!()
}
