use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    path::{Path, PathBuf},
};

use relative_path::{RelativePath, RelativePathBuf};

use crate::monja::AbsolutePath;
use crate::monja::local;

pub(crate) struct Repo {
    sets: HashMap<SetName, Set>,
}

impl Repo {
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

    pub(crate) fn get_set(&self, set_name: &SetName) -> Option<&Set> {
        self.sets.get(set_name)
    }

    pub(crate) fn set_count(&self) -> usize {
        self.sets.len()
    }
}

#[derive(Hash, PartialEq, Eq)]
pub(crate) struct ObjectPath {
    path: RelativePathBuf,
    local_path: local::FilePath,
}
impl ObjectPath {
    fn new(set_name: &SetName, root: &RelativePath, object_path: &RelativePath) -> ObjectPath {
        let mut path = RelativePathBuf::new();
        path.push(&set_name.0);
        path.push(root);
        path.push(object_path);

        let mut local_path = RelativePathBuf::new();
        local_path.push(root);
        local_path.push(object_path);

        ObjectPath {
            path,
            local_path: local::FilePath::new(local_path),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone)]
pub(crate) struct SetName(String);
impl Display for SetName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub(crate) struct Set {
    root: RelativePathBuf,
    absolute_root: AbsolutePath,
    directories: HashMap<ObjectPath, Directory>,
    files: HashMap<ObjectPath, File>,
    locally_mapped_files: HashMap<local::FilePath, File>,
}
impl Set {
    pub(crate) fn tracks_file(&self, local_file: &local::FilePath) -> bool {
        // TODO: hashset?
        self.locally_mapped_files.contains_key(local_file)
    }

    pub(crate) fn absolute_root(&self) -> &AbsolutePath {
        &self.absolute_root
    }

    pub(crate) fn relative_root(&self) -> &RelativePath {
        &self.root
    }
}
//  TODO: am expecting this design to become necessary, though it isnt right now
// in particular, we might track cleanup state through this
pub(crate) struct File {
    repo_path: PathBuf,
    local_path: PathBuf,
}
impl File {}

pub(crate) struct Directory {
    repo_path: PathBuf,
    files: Vec<File>,
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

pub(crate) fn initialize_full_state(root: &Path) -> Repo {
    todo!()
}
