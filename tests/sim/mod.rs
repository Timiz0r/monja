use std::{
    fs,
    ops::Deref,
    path::{Path, PathBuf},
};

use monja::{AbsolutePath, MonjaProfile, SetName};
use tempfile::TempDir;

pub(crate) struct Simulator {
    repo_dir: FsBuilder,
    local_dir: FsBuilder,
    profile: MonjaProfile,
}

impl Simulator {
    pub(crate) fn create() -> Self {
        let repo_dir = FsBuilder::create("repo");
        let local_dir = FsBuilder::create("local");
        let profile = MonjaProfile {
            repo_root: AbsolutePath::new(repo_dir.tempdir.path().to_path_buf()).unwrap(),
            local_root: AbsolutePath::new(local_dir.tempdir.path().to_path_buf()).unwrap(),
            target_sets: vec![],
            new_file_set: None,
        };

        Simulator {
            repo_dir: FsBuilder::create("repo"),
            local_dir: FsBuilder::create("local"),
            profile,
        }
    }

    pub(crate) fn profile(&self) -> &MonjaProfile {
        &self.profile
    }

    pub(crate) fn new_file_set(&mut self, set_name: Option<SetName>) -> &mut Self {
        self.profile.new_file_set = set_name;
        self
    }

    pub(crate) fn set<B>(&mut self, name: SetName, builder: B) -> &mut Self
    where
        B: FnMut(DirBuilder) -> DirBuilder,
    {
        self.repo_dir.dir(name.0, builder);
        self
    }

    pub(crate) fn rem_set(&mut self, name: SetName) -> &mut Self {
        self.repo_dir.rem_dir(name.0);

        self
    }
}

pub(crate) struct FsBuilder {
    tempdir: TempDir,
    // with deref, saves duplicate functions
    dir_builder: DirBuilder,
}
impl FsBuilder {
    pub(crate) fn create(prefix: &str) -> Self {
        let tempdir = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
        FsBuilder {
            dir_builder: DirBuilder {
                path: tempdir.path().to_path_buf(),
            },
            tempdir,
        }
    }
}

impl Deref for FsBuilder {
    type Target = DirBuilder;

    fn deref(&self) -> &Self::Target {
        &self.dir_builder
    }
}

pub(crate) struct DirBuilder {
    path: PathBuf,
}
impl DirBuilder {
    pub(crate) fn dir<P, B>(&self, path: P, mut builder: B) -> &Self
    where
        P: AsRef<Path>,
        B: FnMut(DirBuilder) -> DirBuilder,
    {
        let path = self.path.join(path.as_ref());
        fs::create_dir_all(&path).unwrap();

        let dir_builder = DirBuilder { path };
        builder(dir_builder);

        self
    }

    pub(crate) fn rem_dir<P>(&self, path: P) -> &Self
    where
        P: AsRef<Path>,
    {
        let path = self.path.join(path.as_ref());
        fs::remove_dir_all(path).unwrap();

        self
    }

    pub(crate) fn file<C: AsRef<[u8]>>(&self, name: &str, contents: C) -> &Self {
        fs::write(self.path.join(name), contents).unwrap();

        self
    }

    pub(crate) fn rem_file(&self, name: &str) -> &Self {
        fs::remove_file(self.path.join(name)).unwrap();

        self
    }
}
