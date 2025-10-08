use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    path::PathBuf,
};

use relative_path::{RelativePath, RelativePathBuf};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{AbsolutePath, local};

pub(crate) struct Repo {
    sets: HashMap<SetName, Set>,
}

// TODO: might as well deref to str and add a from
#[derive(PartialEq, Eq, Hash, Clone, Debug, Serialize, Deserialize)]
pub struct SetName(pub String);
impl Display for SetName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
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

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SetConfig {
    // used to be called root, but it was hard to disambiguate with other uses of the term
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shortcut: Option<PathBuf>,
}

pub(crate) struct Set {
    name: SetName,
    // TODO: validate and test that this is a child of the local root, to avoid directory traversal attacks
    shortcut: RelativePathBuf,
    root: AbsolutePath,
    // directories: HashMap<ObjectPath, Directory>,
    locally_mapped_files: HashMap<local::FilePath, File>,
}
impl Set {
    pub(crate) fn name(&self) -> &SetName {
        &self.name
    }

    pub(crate) fn tracks_file(&self, local_file: &local::FilePath) -> bool {
        self.locally_mapped_files.contains_key(local_file)
    }

    pub(crate) fn root(&self) -> &AbsolutePath {
        &self.root
    }

    pub(crate) fn shortcut(&self) -> &RelativePath {
        &self.shortcut
    }

    pub(crate) fn files(&self) -> impl Iterator<Item = &File> {
        self.locally_mapped_files.values()
    }
}

pub(crate) struct FilePath {
    path_in_set: RelativePathBuf,
    local_path: local::FilePath,
}

impl FilePath {
    fn new(shortcut: &RelativePath, path_in_set: RelativePathBuf) -> FilePath {
        let mut local_path = RelativePathBuf::new();
        local_path.push(shortcut);
        local_path.push(&path_in_set);
        let local_path = local::FilePath::new(local_path);

        FilePath {
            path_in_set,
            local_path,
        }
    }

    // choosing to encapsulate since we have strict logic for instantiation
    pub(crate) fn path_in_set(&self) -> &RelativePathBuf {
        &self.path_in_set
    }

    pub(crate) fn local_path(&self) -> &local::FilePath {
        &self.local_path
    }
}

// TODO: am expecting this design to become necessary, though it isnt right now
// for instance, we might track perms or cleanup state through these
// additionally, the directory struct might become the representation of a future monja-dir.toml
pub(crate) struct File {
    pub owning_set: SetName,
    pub path: FilePath,
}
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
        let set_config = match std::fs::read(set_path.join(".monja-set.toml")) {
            Ok(raw) => toml::from_slice(&raw).unwrap(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => SetConfig::default(),
            Err(e) => panic!("{}", e), //TODO: dont forget about this. keyword unwrap
        };

        let shortcut = set_config.shortcut.unwrap_or("".into());
        let shortcut = RelativePathBuf::from_path(shortcut).unwrap();

        let mut locally_mapped_files = HashMap::new();
        for entry in WalkDir::new(&set_path) {
            match entry {
                Ok(entry) if entry.file_type().is_file() => {
                    let path_in_set = entry
                        .path()
                        .strip_prefix(&set_path)
                        .expect("The entry path should start with set_path, since that's what we called it with.");
                    let path_in_set = RelativePathBuf::from_path(path_in_set)
                        .expect("Stripping of the prefix should make path relative");
                    let path = FilePath::new(&shortcut, path_in_set);

                    let file = File {
                        owning_set: set_name.clone(),
                        path,
                    };

                    locally_mapped_files.insert(file.path.local_path().clone(), file);
                }
                Err(err) => errors.push(err),
                _ => {}
            };
        }

        let root = AbsolutePath::from_path(root.join(&set_name.0)).unwrap();
        let set = Set {
            name: set_name.clone(),
            shortcut,
            root,
            locally_mapped_files,
        };
        sets.insert(set_name, set);
    }
    let sets = sets;
    let errors = errors;

    if !errors.is_empty() {
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
