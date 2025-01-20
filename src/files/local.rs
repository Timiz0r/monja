use ignore::WalkBuilder;
use std::{
    io,
    path::{Path, PathBuf},
};

struct IgnoreFile {}
impl IgnoreFile {
    fn load(path: &Path) -> IgnoreFile {
        todo!()
    }
}

pub(crate) struct File {
    path: PathBuf,
}
impl File {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn get_permissions(&self) -> Vec<exacl::AclEntry> {
        exacl::getfacl(&self.path, None).unwrap()
    }
}

pub(crate) fn walk<'a>(root: &'a Path) -> impl Iterator<Item = io::Result<File>> + use<> {
    // TODO: figure out error
    let walker = WalkBuilder::new(&root)
        .standard_filters(false)
        .add_custom_ignore_filename(".monjaignore")
        .follow_links(true)
        .build();
    walker.flatten().map(|entry| {
        Ok(File {
            path: entry.into_path(),
        })
    })
}
