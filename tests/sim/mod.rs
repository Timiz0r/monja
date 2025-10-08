use std::{
    fs,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

use googletest::prelude::*;

use monja::{AbsolutePath, MonjaProfile, SetName};
use tempfile::TempDir;

// the types here use mutability to indicate file system operations,
// which is incidentally why we pass references to DirBuilder (sometimes mut), instead of value.
// it's also why file operations require a mutable reference, even though nothing is actually mutated.
//
// granted, this is just a simulator, so it wouldnt be a big deal
// if verification operations allowed file operations.
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
            repo_root: AbsolutePath::from_path(repo_dir.tempdir.path().to_path_buf()).unwrap(),
            local_root: AbsolutePath::from_path(local_dir.tempdir.path().to_path_buf()).unwrap(),
            target_sets: vec![],
            new_file_set: None,
        };

        Simulator {
            repo_dir,
            local_dir,
            profile,
        }
    }

    pub(crate) fn profile(&self) -> &MonjaProfile {
        &self.profile
    }

    // pass by value to move old profile
    pub(crate) fn configure_profile<P>(mut self, mut config: P) -> Self
    where
        P: FnMut(MonjaProfile) -> MonjaProfile,
    {
        self.profile = config(self.profile);
        self
    }

    pub(crate) fn set<B>(&mut self, name: SetName, builder: B) -> &mut Self
    where
        B: FnMut(&mut DirBuilder),
    {
        self.repo_dir.dir(name.0, builder);
        self
    }

    pub(crate) fn rem_set(&mut self, name: SetName) -> &mut Self {
        self.repo_dir.rem_dir(name.0);

        self
    }

    // main difference to set() is lack of mutability
    pub(crate) fn validate_set<B>(&self, name: SetName, builder: B) -> &Self
    where
        B: FnMut(&DirBuilder),
    {
        // this also takes care of making sure the dir exists, since it's now part of the path
        self.repo_dir.validate_dir(name.0, builder);

        self
    }
}

pub(crate) struct FsBuilder {
    tempdir: TempDir,
    // with deref, saves code duplication
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

impl DerefMut for FsBuilder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.dir_builder
    }
}

pub(crate) struct DirBuilder {
    path: PathBuf,
}
impl DirBuilder {
    pub(crate) fn dir<P, B>(&mut self, path: P, mut builder: B) -> &mut Self
    where
        P: AsRef<Path>,
        B: FnMut(&mut DirBuilder),
    {
        let path = self.path.join(path.as_ref());
        fs::create_dir_all(&path).unwrap();

        let mut dir_builder = DirBuilder { path };
        builder(&mut dir_builder);

        self
    }

    pub(crate) fn rem_dir<P>(&mut self, path: P) -> &mut Self
    where
        P: AsRef<Path>,
    {
        let path = self.path.join(path.as_ref());
        fs::remove_dir_all(path).unwrap();

        self
    }

    pub(crate) fn validate_dir<P, B>(&self, path: P, mut builder: B) -> &Self
    where
        P: AsRef<Path>,
        B: FnMut(&DirBuilder),
    {
        let path = self.path.join(path.as_ref());

        let dir = fs::read_dir(&path);
        expect_that!(dir, ok(anything()));
        // hypothetically, we could stop here, since sub directories and files definitely dont exist,
        // but we'll keep going for logging reasons

        let dir_builder = DirBuilder { path };
        builder(&dir_builder);

        self
    }

    pub(crate) fn file<C: AsRef<[u8]>>(&mut self, name: &str, contents: C) -> &mut Self {
        fs::write(self.path.join(name), contents).unwrap();

        self
    }

    pub(crate) fn rem_file(&mut self, name: &str) -> &mut Self {
        fs::remove_file(self.path.join(name)).unwrap();

        self
    }

    pub(crate) fn validate_file<C: AsRef<[u8]>>(&self, name: &str, expected_contents: C) -> &Self {
        let contents = fs::read(self.path.join(name)).unwrap();

        expect_that!(contents, container_eq(expected_contents.as_ref().to_vec()));

        self
    }
}
