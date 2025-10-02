use crate::monja::{AbsolutePath, repo};
use ignore::WalkBuilder;
use relative_path::{RelativePath, RelativePathBuf};
use std::{collections::HashMap, io};

struct FileIndex {
    set_mapping: HashMap<FilePath, repo::SetName>,
}
impl FileIndex {
    fn load(root: &AbsolutePath) -> FileIndex {
        todo!()
    }
    fn get(&self, path: &FilePath) -> Option<&repo::SetName> {
        self.set_mapping.get(path)
    }
}

#[derive(Hash, PartialEq, Eq)]
pub(crate) struct FilePath {
    path: RelativePathBuf,
}
impl FilePath {
    pub(crate) fn new(object_path: RelativePathBuf) -> FilePath {
        FilePath { path: object_path }
    }
    pub(crate) fn relative_path(&self) -> &RelativePath {
        &self.path
    }
}

// TODO: do a pass on encapsulation for all structs
pub struct MonjaProfile {
    local_root: AbsolutePath,
    repo_root: AbsolutePath,
    target_sets: Vec<repo::SetName>,
    new_file_set: repo::SetName,
}

impl MonjaProfile {
    pub(crate) fn repo_root(&self) -> &AbsolutePath {
        &self.repo_root
    }

    pub(crate) fn local_root(&self) -> &AbsolutePath {
        &self.local_root
    }
}

pub(crate) struct LocalState {
    pub files_to_push: Vec<(repo::SetName, Vec<FilePath>)>,
    pub untracked_files: Vec<FilePath>,
    pub missing_sets: Vec<(repo::SetName, Vec<FilePath>)>,
    pub missing_files: Vec<(repo::SetName, Vec<FilePath>)>,
}

impl LocalState {
    // a previous implementation of these returned Option(impl Iterator...).
    // however, to support multiple iteration, we no longer do this.
    // nested slices arent viable because of the nested vectors -- the outer needs some sort of allocation.
    // instead, we'll just expose the mutable vecs and expect them to be unmodified.

    // TODO: remove old implementation once in at least one git commit
    // for these implementations, we use iterators and options because we need to know if they're empty
    // and using slices for nested vectors isnt viable, at least without boxes
    // admittedly not sure whats better design-wise, but at least this works fine
    // pub(crate) fn files_to_push(
    //     &self,
    // ) -> Option<impl Iterator<Item = (&repo::SetName, impl Iterator<Item = &FilePath>)>> {
    //     // TODO: learn macros
    //     if self.files_to_push.is_empty() {
    //         None
    //     } else {
    //         Some(
    //             self.files_to_push
    //                 .iter()
    //                 .map(|pair| (&pair.0, pair.1.iter())),
    //         )
    //     }
    // }

    // pub(crate) fn untracked_files(&self) -> Option<impl Iterator<Item = &FilePath>> {
    //     if self.untracked_files.is_empty() {
    //         None
    //     } else {
    //         Some(self.untracked_files.iter())
    //     }
    // }

    // pub(crate) fn missing_sets(
    //     &self,
    // ) -> Option<impl Iterator<Item = (&repo::SetName, impl Iterator<Item = &FilePath>)>> {
    //     if self.missing_sets.is_empty() {
    //         None
    //     } else {
    //         Some(
    //             self.missing_sets
    //                 .iter()
    //                 .map(|pair| (&pair.0, pair.1.iter())),
    //         )
    //     }
    // }
    // pub(crate) fn missing_files(
    //     &self,
    // ) -> Option<impl Iterator<Item = (&repo::SetName, impl Iterator<Item = &FilePath>)>> {
    //     if self.missing_files.is_empty() {
    //         None
    //     } else {
    //         Some(
    //             self.missing_files
    //                 .iter()
    //                 .map(|pair| (&pair.0, pair.1.iter())),
    //         )
    //     }
    // }
}

pub(crate) fn retrieve_state(profile: &MonjaProfile, repo: &repo::Repo) -> LocalState {
    let index = FileIndex::load(&profile.local_root);

    let mut files_to_push = HashMap::with_capacity(repo.set_count());
    let mut untracked_files = Vec::new();
    // so signifies the files indicating the set should exist
    let mut missing_sets = HashMap::with_capacity(repo.set_count());
    let mut missing_files = HashMap::with_capacity(repo.set_count());

    for local_path in walk(&profile.local_root) {
        let local_path = local_path.unwrap();
        let Some(set_name) = index.get(&local_path) else {
            untracked_files.push(local_path);
            continue;
        };
        // note that single clone works thanks to early exit continues
        let set_name = set_name.clone();
        let Some(set) = repo.get_set(&set_name) else {
            missing_sets
                .entry(set_name)
                .or_insert_with(Vec::new)
                .push(local_path);
            continue;
        };
        if !set.tracks_file(&local_path) {
            missing_files
                .entry(set_name)
                .or_insert_with(Vec::new)
                .push(local_path);
            continue;
        }

        files_to_push
            .entry(set_name)
            .or_insert_with(Vec::new)
            .push(local_path);
    }

    LocalState {
        files_to_push: files_to_push.into_iter().collect(),
        untracked_files,
        missing_sets: missing_sets.into_iter().collect(),
        missing_files: missing_files.into_iter().collect(),
    }
}

fn walk(root: &AbsolutePath) -> impl Iterator<Item = io::Result<FilePath>> {
    let walker = WalkBuilder::new(root)
        .standard_filters(false)
        .add_custom_ignore_filename(".monjaignore")
        .follow_links(true)
        .build();
    walker.flatten().map(|entry| {
        Ok(FilePath {
            path: RelativePathBuf::from_path(entry.path()).unwrap(),
        })
    })
}
