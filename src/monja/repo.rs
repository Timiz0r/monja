use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    path::Path,
};

use relative_path::{RelativePath, RelativePathBuf};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

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

#[derive(PartialEq, Eq, Hash, Clone, Serialize, Deserialize)]
pub(crate) struct SetName(String);
impl Display for SetName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub(crate) struct Set {
    root: RelativePathBuf,
    absolute_root: AbsolutePath,
    // directories: HashMap<ObjectPath, Directory>,
    // files: HashMap<ObjectPath, File>,
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

// TODO: am expecting this design to become necessary, though it isnt right now
// for instance, we might track cleanup state through this
// additionally, the directory struct might become the representation of a future monja-dir.toml
pub(crate) struct File {}
impl File {}

pub(crate) struct Directory {}
impl Directory {}

pub(crate) fn initialize_full_state(root: &AbsolutePath) -> std::io::Result<Repo> {
    let read_dir = std::fs::read_dir(root)?;
    let mut set_info = vec![];
    let mut errors = vec![];

    // versus ::partition, less vector allocations, since we can unwrap results here
    for result in read_dir {
        match result {
            Err(err) => errors.push(err),
            Ok(res) if res.path().is_dir() => {
                match res.file_name().to_str() {
                    Some(str) => set_info.push((SetName(str.to_string()), res.path())),
                    // this is so unlikely to happen that a static error is sufficient
                    None => errors.push(std::io::Error::other(
                        "Unable to convert dir name into set name.",
                    )),
                };
            }
            _ => {}
        };
    }
    let set_info = set_info;
    let errors = errors;

    if !errors.is_empty() {
        // TODO: when we're doing custom errors, use all collected errors
        return Err(errors
            .into_iter()
            .next()
            .expect("Already checked that the errors vector isn't empty"));
    }

    let mut sets = HashMap::with_capacity(set_info.len());
    let mut errors = vec![];
    for (set_name, set_path) in set_info {
        let mut set = Set {
            root: "todo!()".into(),
            absolute_root: AbsolutePath::new("todo!()".into()).unwrap(),
            locally_mapped_files: HashMap::new(),
        };

        for entry in WalkDir::new(&set_path) {
            match entry {
                Ok(entry) => _ = set.locally_mapped_files.insert(
                    local::FilePath::new(
                        RelativePathBuf::from_path(
                            entry
                                .path()
                                .strip_prefix(&set_path)
                                .expect("The entry path should start with set_path, since that's what we called it with."))
                        .expect("Stripping of the prefix should make path relative"),
                    ),
                    File { },
                ),
                Err(err) => errors.push(err),
            };
        }

        sets.insert(set_name, set);
    }
    let sets = sets;
    let errors = errors;

    if errors.is_empty() {
        // TODO: all errors
        return Err(errors
            .into_iter()
            .next()
            .expect("Already checked that the errors vector isn't empty")
            .into());
    }

    Ok(Repo { sets })
}

// fn set_state(root: &AbsolutePath, )
