use std::{fs, path::Path};

use monja::{AbsolutePath, MonjaProfile, MonjaProfileConfig, SetConfig, SetName};

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
        let local_root = AbsolutePath::from_path(self.local_dir.path()).unwrap();

        MonjaProfileConfig::load(&self.profile_path).into_config(local_root)
    }

    // pass by value to move old profile
    pub(crate) fn configure_profile<P>(&mut self, mut config: P) -> &mut Self
    where
        P: FnMut(MonjaProfileConfig) -> MonjaProfileConfig,
    {
        let profile_config = config(self.profile().config);
        profile_config.save(&self.profile_path);

        self
    }

    pub(crate) fn configure_set<P>(&self, set_name: SetName, mut config: P) -> &Self
    where
        P: FnMut(SetConfig) -> SetConfig,
    {
        let profile = self.profile();
        let set_config = SetConfig::load(&profile, &set_name);
        let set_config = config(set_config);

        set_config.save(&profile, &set_name);

        self
    }

    // adding is handled by set_operation!
    pub(crate) fn rem_set(&mut self, name: SetName) -> &mut Self {
        let path = self.repo_dir.path().join(name.0);
        SetManipulation::remove_dir(path);

        self
    }
}

pub(crate) trait OperationHandler {
    fn dir<P>(path: P)
    where
        P: AsRef<Path>;
    fn remove_dir<P>(path: P)
    where
        P: AsRef<Path>;

    fn file<P, C>(path: P, contents: C)
    where
        P: AsRef<Path>,
        C: AsRef<str>;
    fn remove_file<P, C>(path: P)
    where
        P: AsRef<Path>;
}

pub(crate) struct SetManipulation;
impl OperationHandler for SetManipulation {
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
        C: AsRef<str>,
    {
        fs::write(path, contents.as_ref()).unwrap();
    }

    fn remove_file<P, C>(path: P)
    where
        P: AsRef<Path>,
    {
        fs::remove_file(path).unwrap();
    }
}

pub(crate) struct LocalValidation;
impl OperationHandler for LocalValidation {
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
        C: AsRef<str>,
    {
        let contents = fs::read_to_string(path).unwrap();
        expect_that!(contents, eq(expected_contents.as_ref()));
    }

    fn remove_file<P, C>(_path: P)
    where
        P: AsRef<Path>,
    {
        panic!("Not possible to remove_file for validation.")
    }
}

// a previous version used lambda-based (nested) builders to do the same thing.
// while reasonably readable, it was also somewhat hard to read and get parenthesis matched up correctly
// though macros have horrible diagnostics when there's an issue, it overall works well!
#[allow(unused_macros)]
macro_rules! fs_operation {
    // largely follows https://danielkeep.github.io/tlborm/book/aeg-ook.html

    // these don't really need to be last, since the internal stuff won't match this.
    // so putting it at the top to stand out more
    (SetManipulation, $sim:expr, $set:literal, $($tokens:tt)*) => {
        {
            let path = $sim.profile().repo_root.join($set);
            fs_operation!(@start (SetManipulation, path); ($($tokens)*));
        }
    };

    (LocalValidation, $sim:expr, $($tokens:tt)*) => {
        {
            let path = $sim.profile().local_root;
            fs_operation!(@start (LocalValidation, path); ($($tokens)*));
        }
    };

    (@start ($handler:ident, $path_var:ident); $tokens:tt) => {
        <$handler as $crate::sim::OperationHandler>::dir(&$path_var);
        fs_operation!(@general ($handler, $path_var); $tokens);
    };

    (@general ($handler:ident, $path_var:ident); (file $path:literal $contents:literal $($tail:tt)*)) => {
        {
            let path = $path_var.join($path);
            <$handler as $crate::sim::OperationHandler>::file(&path, $contents);
        }

        fs_operation!(@general ($handler, $path_var); ($($tail)*));
    };

    (@general ($handler:ident, $path_var:ident); (remfile $path:literal $($tail:tt)*)) => {
        {
            let path = $path_var.join($path);
            <$handler as $crate::sim::OperationHandler>::remove_file(path);
        }

        fs_operation!(@general ($handler, $path_var); ($($tail)*));
    };

    (@general ($handler:ident, $path_var:ident); (remdir $path:literal $($tail:tt)*)) => {
        {
            let path = $path_var.join($path);
            <$handler as $crate::sim::OperationHandler>::remove_dir(path);
        }

        fs_operation!(@general ($handler, $path_var); ($($tail)*));
    };

    (@general ($handler:ident, $path_var:ident); (dir $path:literal $($tail:tt)*)) => {
        {
            let path = $path_var.join($path);
            <$handler as $crate::sim::OperationHandler>::dir(&path);

            fs_operation!(@extract_inner ($handler, path); (); (); ($($tail)*));
        }
        fs_operation!(@skip_to_outer ($handler, $path_var); (); ($($tail)*));
    };

    (@general $symbols:tt; ()) => {};

    // if we hit `end` with depth 0, take the $buf and run it
    (@extract_inner $symbols:tt; (); ($($buffer:tt)*); (end $($tail:tt)*)) => {
        fs_operation!(@general $symbols; ($($buffer)*));
    };

    // controls depth, based on subsequent `dir`s and matching `end`s
    // we need to add them into the buffer because they are to be evaluated with the about depth-0 `end`
    (@extract_inner $symbols:tt; ($($depth:tt)*); ($($buffer:tt)*); (dir $path:literal $($tail:tt)*)) => {
        fs_operation!(@extract_inner $symbols; (@ $($depth)*); ($($buffer)* dir $path); ($($tail)*));
    };
    (@extract_inner $symbols:tt; (@ $($depth:tt)*); ($($buffer:tt)*); (end $($tail:tt)*)) => {
        fs_operation!(@extract_inner $symbols; ($($depth)*); ($($buffer)* end); ($($tail)*));
    };

    // the example has a pretty strict grammar, which makes the "catch-all" easy.
    // the only way I can think to do it for our grammar is with multiple cases
    (@extract_inner $symbols:tt; $depth:tt; ($($buffer:tt)*); (file $path:literal $contents:literal $($tail:tt)*)) => {
        fs_operation!(@extract_inner $symbols; $depth; ($($buffer)* file $path $contents); ($($tail)*));
    };
    (@extract_inner $symbols:tt; $depth:tt; ($($buffer:tt)*); (remfile $path:literal $($tail:tt)*)) => {
        fs_operation!(@extract_inner $symbols; $depth; ($($buffer)* file $path); ($($tail)*));
    };
    (@extract_inner $symbols:tt; $depth:tt; ($($buffer:tt)*); (remdir $path:literal $($tail:tt)*)) => {
        fs_operation!(@extract_inner $symbols; $depth; ($($buffer)* file $path); ($($tail)*));
    };

    // and now the same sort of ones for skipping
    (@skip_to_outer $symbols:tt; (); (end $($tail:tt)*)) => {
        fs_operation!(@general $symbols; ($($tail)*));
    };

    (@skip_to_outer $symbols:tt; ($($depth:tt)*); (dir $path:literal $($tail:tt)*)) => {
        fs_operation!(@skip_to_outer $symbols; (@ $($depth)*); ($($tail)*));
    };
    (@skip_to_outer $symbols:tt; (@ $($depth:tt)*); (end $($tail:tt)*)) => {
        fs_operation!(@skip_to_outer $symbols; ($($depth)*); ($($tail)*));
    };

    (@skip_to_outer $symbols:tt; $depth:tt; (file $path:literal $contents:literal $($tail:tt)*)) => {
        fs_operation!(@skip_to_outer $symbols; $depth; ($($tail)*));
    };
    (@skip_to_outer $symbols:tt; $depth:tt; (remfile $path:literal $($tail:tt)*)) => {
        fs_operation!(@skip_to_outer $symbols; $depth; ($($tail)*));
    };
    (@skip_to_outer $symbols:tt; $depth:tt; (remdir $path:literal $($tail:tt)*)) => {
        fs_operation!(@skip_to_outer $symbols; $depth; ($($tail)*));
    };
}
