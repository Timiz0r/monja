use std::{fs, path::Path};

use monja::{AbsolutePath, MonjaProfile, MonjaProfileConfig, SetName};

use googletest::prelude::*;
use tempfile::TempDir;

// the types here use mutability to indicate file system operations,
// which is incidentally why we pass references to DirBuilder (sometimes mut), instead of value.
// it's also why file operations require a mutable reference, even though nothing is actually mutated.
//
// granted, this is just a simulator, so it wouldnt be a big deal
// if verification operations allowed file operations.
pub(crate) struct Simulator {
    repo_dir: TempDir,
    local_dir: TempDir,
    profile_path: AbsolutePath,
}

impl Simulator {
    pub(crate) fn create() -> Self {
        let repo_dir = tempfile::Builder::new()
            .prefix("MonjaRepo")
            .tempdir()
            .unwrap();
        let local_dir = tempfile::Builder::new()
            .prefix("MonjaLocal")
            .tempdir()
            .unwrap();

        let profile_config = MonjaProfileConfig {
            monja_dir: repo_dir.path().to_path_buf(),
            target_sets: vec![],
            new_file_set: None,
        };

        let profile_path = local_dir.path().join(".monja-profile.toml");
        // AbsolutePath requires that the path exist, so we'll make it first.
        // typically, the profile should already be user-created in practice, so this behavior is desireable.
        // also, since this is a fresh dir, no overwriting happens here.
        fs::write(&profile_path, "").unwrap();
        let profile_path =
            AbsolutePath::from_path(local_dir.path().join(".monja-profile.toml")).unwrap();
        profile_config.save(&profile_path);

        Simulator {
            repo_dir,
            local_dir,
            profile_path,
        }
    }

    pub(crate) fn profile(&self) -> MonjaProfile {
        // we previously stored an instance of the profile
        // however, we changed it to reading a file to get coverage of the code paths
        let local_root = AbsolutePath::from_path(self.local_dir.path().to_path_buf()).unwrap();

        MonjaProfileConfig::load(&self.profile_path).into_config(local_root)
    }

    // pass by value to move old profile
    pub(crate) fn configure_profile<P>(self, mut config: P) -> Self
    where
        P: FnMut(MonjaProfile) -> MonjaProfile,
    {
        let profile = config(self.profile());
        profile.save_config(&self.profile_path);

        self
    }

    // adding is handled by set_operation!
    pub(crate) fn rem_set(&mut self, name: SetName) -> &mut Self {
        let path = self.repo_dir.path().join(name.0);
        Manipulate::remove_dir(path);

        self
    }
}

pub(crate) trait Handler {
    fn dir<P>(path: P)
    where
        P: AsRef<Path>;
    fn remove_dir<P>(path: P)
    where
        P: AsRef<Path>;

    fn file<P, C>(path: P, contents: C)
    where
        P: AsRef<Path>,
        C: AsRef<[u8]>;
    fn remove_file<P, C>(path: P)
    where
        P: AsRef<Path>,
        C: AsRef<[u8]>;
}

pub(crate) struct Manipulate;
impl Handler for Manipulate {
    fn dir<P>(path: P)
    where
        P: AsRef<Path>,
    {
        fs::create_dir_all(path).unwrap();
    }

    fn remove_dir<P>(path: P)
    where
        P: AsRef<Path>,
    {
        fs::remove_dir_all(path).unwrap();
    }

    fn file<P, C>(path: P, contents: C)
    where
        P: AsRef<Path>,
        C: AsRef<[u8]>,
    {
        fs::write(path, contents).unwrap();
    }

    fn remove_file<P, C>(path: P)
    where
        P: AsRef<Path>,
        C: AsRef<[u8]>,
    {
        fs::remove_file(path).unwrap();
    }
}

pub(crate) struct Validate;
impl Handler for Validate {
    fn dir<P>(path: P)
    where
        P: AsRef<Path>,
    {
        let dir = fs::read_dir(&path);
        expect_that!(dir, ok(anything()));
    }

    fn remove_dir<P>(_path: P)
    where
        P: AsRef<Path>,
    {
        panic!("Not possible to remove_dir for validation.")
    }

    fn file<P, C>(path: P, expected_contents: C)
    where
        P: AsRef<Path>,
        C: AsRef<[u8]>,
    {
        let contents = fs::read(path).unwrap();
        expect_that!(contents, container_eq(expected_contents.as_ref().to_vec()));
    }

    fn remove_file<P, C>(_path: P)
    where
        P: AsRef<Path>,
        C: AsRef<[u8]>,
    {
        panic!("Not possible to remove_file for validation.")
    }
}

// a previous version used lambda-based (nested) builders to do the same thing.
// while reasonably readable, it was also somewhat hard to read and get parenthesis matched up correctly
// though macros have horrible diagnostics when there's an issue, it overall works well!
#[allow(unused_macros)]
macro_rules! set_operation {
    // largely follows https://danielkeep.github.io/tlborm/book/aeg-ook.html

    // this doesn't really need to be last, since the internal stuff won't match this.
    // so putting it at the top to stand out more
    ($handler:ident, $sim:expr, $set:literal, $($tokens:tt)*) => {
        {
            let path = $sim.profile().repo_root.join($set);
            <$handler as $crate::sim::Handler>::dir(&path);
            set_operation!(@general ($handler, path); ($($tokens)*));
        }
    };

    (@general ($handler:ident, $path_var:ident); (file $path:literal $contents:literal $($tail:tt)*)) => {
        {
            let path = $path_var.join($path);
            <$handler as $crate::sim::Handler>::file(&path, $contents);
        }

        set_operation!(@general ($handler, $path_var); ($($tail)*));
    };

    (@general ($handler:ident, $path_var:ident); (remfile $path:literal $($tail:tt)*)) => {
        {
            let path = $path_var.join($path);
            <$handler as $crate::sim::Handler>::remove_file(path);
        }

        set_operation!(@general ($handler, $path_var); ($($tail)*));
    };

    (@general ($handler:ident, $path_var:ident); (remdir $path:literal $($tail:tt)*)) => {
        {
            let path = $path_var.join($path);
            <$handler as $crate::sim::Handler>::remove_dir(path);
        }

        set_operation!(@general ($handler, $path_var); ($($tail)*));
    };

    (@general ($handler:ident, $path_var:ident); (dir $path:literal $($tail:tt)*)) => {
        {
            let path = $path_var.join($path);
            <$handler as $crate::sim::Handler>::dir(&path);

            set_operation!(@extract_inner ($handler, $path_var); (); (); ($($tail)*));
        }
        set_operation!(@skip_to_outer ($handler, $path_var); (); ($($tail)*));
    };

    (@general $symbols:tt; ()) => {};

    // if we hit `end` with depth 0, take the $buf and run it
    (@extract_inner $symbols:tt; (); ($($buffer:tt)*); (end $($tail:tt)*)) => {
        set_operation!(@general $symbols; ($($buffer)*));
    };

    // controls depth, based on subsequent `dir`s and matching `end`s
    // we need to add them into the buffer because they are to be evaluated with the about depth-0 `end`
    (@extract_inner $symbols:tt; ($($depth:tt)*); ($($buffer:tt)*); (dir $path:literal $($tail:tt)*)) => {
        set_operation!(@extract_inner $symbols; (@ $($depth)*); ($($buffer)* dir $path); ($($tail)*));
    };
    (@extract_inner $symbols:tt; (@ $($depth:tt)*); ($($buffer:tt)*); (end $($tail:tt)*)) => {
        set_operation!(@extract_inner $symbols; ($($depth)*); ($($buffer)* end); ($($tail)*));
    };

    // the example has a pretty strict grammar, which makes the "catch-all" easy.
    // the only way I can think to do it for our grammar is with multiple cases
    (@extract_inner $symbols:tt; $depth:tt; ($($buffer:tt)*); (file $path:literal $contents:literal $($tail:tt)*)) => {
        set_operation!(@extract_inner $symbols; $depth; ($($buffer)* file $path $contents); ($($tail)*));
    };
    (@extract_inner $symbols:tt; $depth:tt; ($($buffer:tt)*); (remfile $path:literal $($tail:tt)*)) => {
        set_operation!(@extract_inner $symbols; $depth; ($($buffer)* file $path); ($($tail)*));
    };
    (@extract_inner $symbols:tt; $depth:tt; ($($buffer:tt)*); (remdir $path:literal $($tail:tt)*)) => {
        set_operation!(@extract_inner $symbols; $depth; ($($buffer)* file $path); ($($tail)*));
    };

    // and now the same sort of ones for skipping
    (@skip_to_outer $symbols:tt; (); (end $($tail:tt)*)) => {
        set_operation!(@general $symbols; ($($tail)*));
    };

    (@skip_to_outer $symbols:tt; ($($depth:tt)*); (dir $path:literal $($tail:tt)*)) => {
        set_operation!(@skip_to_outer $symbols; (@ $($depth)*); ($($tail)*));
    };
    (@skip_to_outer $symbols:tt; (@ $($depth:tt)*); (end $($tail:tt)*)) => {
        set_operation!(@skip_to_outer $symbols; ($($depth)*); ($($tail)*));
    };

    (@skip_to_outer $symbols:tt; $depth:tt; (file $path:literal $contents:literal $($tail:tt)*)) => {
        set_operation!(@skip_to_outer $symbols; $depth; ($($tail)*));
    };
    (@skip_to_outer $symbols:tt; $depth:tt; (remfile $path:literal $($tail:tt)*)) => {
        set_operation!(@skip_to_outer $symbols; $depth; ($($tail)*));
    };
    (@skip_to_outer $symbols:tt; $depth:tt; (remdir $path:literal $($tail:tt)*)) => {
        set_operation!(@skip_to_outer $symbols; $depth; ($($tail)*));
    };
}
