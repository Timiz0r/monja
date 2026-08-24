use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use googletest::prelude::*;
use tempfile::TempDir;

use monja::{
    AbsolutePath, ExecutionOptions, LocalFilePath, MonjaProfile, MonjaProfileConfig,
    MonjaProfileConfigError, RepoName, SetConfig, SetName,
};
use walkdir::WalkDir;

// NOTE: currently, when testing error cases, many tests inspect the error to ensure it's erroring out for the right reason.
// while this can cause excessive coupling to internal behavior, it's preferable to missing an issue.
// if we find that a test is to fragile, we can of course loosen the matching, or perhaps even come up with a better overall design.

pub(crate) struct Simulator {
    // the first repo is the one `repo_root()` returns, and -- since `create` starts with exactly
    // one -- it stays the implicit default until a test calls `add_repo`.
    repos: Vec<(RepoName, TempDir)>,
    local_root: TempDir,
    data_root: TempDir,

    profile_path: AbsolutePath,
    opts: ExecutionOptions,
}

impl Simulator {
    pub(crate) fn create() -> Self {
        let local_dir = tempfile::Builder::new()
            .prefix("MonjaLocal")
            .tempdir()
            .unwrap();
        let repo_dir = tempfile::Builder::new()
            .prefix("MonjaRepo")
            .tempdir_in(&local_dir)
            .unwrap();
        let data_dir = tempfile::Builder::new()
            .prefix("MonjaData")
            .tempdir_in(&local_dir)
            .unwrap();

        let repos = vec![(RepoName::default_name(), repo_dir)];

        let profile_config = MonjaProfileConfig {
            repos: repos
                .iter()
                .map(|(name, dir)| (name.clone(), dir.path().to_path_buf()))
                .collect(),
            target_sets: Vec::new(),
            ..Default::default()
        };

        let profile_path = local_dir.path().join("monja-profile.toml");
        // AbsolutePath requires that the path exist, so we'll make it first.
        // typically, the profile should already be user-created in practice, so this behavior is desireable.
        // also, since this is a fresh dir, no overwriting happens here.
        fs::write(&profile_path, "").unwrap();
        let profile_path =
            AbsolutePath::for_existing_path(&local_dir.path().join("monja-profile.toml")).unwrap();
        profile_config.save(&profile_path).unwrap();

        Simulator {
            repos,
            local_root: local_dir,
            data_root: data_dir,
            profile_path,
            opts: ExecutionOptions {
                verbosity: 0,
                dry_run: false,
                skip_confirmations: true,
            },
        }
    }

    // adds another repo to the profile. note that this makes the profile multi-repo, so commands
    // that need to pick one (`newset`, `repodir`) will want a `--repo` or a `default-repo`.
    pub(crate) fn add_repo(&mut self, name: &str) -> &Path {
        let name = RepoName::from(name);
        let dir = tempfile::Builder::new()
            .prefix("MonjaRepo")
            .tempdir_in(&self.local_root)
            .unwrap();

        self.repos.push((name.clone(), dir));

        let repos = self
            .repos
            .iter()
            .map(|(name, dir)| (name.clone(), dir.path().to_path_buf()))
            .collect();
        self.configure_profile(|old| MonjaProfileConfig { repos, ..old });

        self.repo_root_for(&name)
    }

    pub(crate) fn repo_root(&self) -> &Path {
        self.repos[0].1.path()
    }

    pub(crate) fn repo_root_for(&self, name: &RepoName) -> &Path {
        self.repos
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, dir)| dir.path())
            .unwrap_or_else(|| panic!("No repo named '{}' in the simulator.", name))
    }

    pub(crate) fn repo_roots(&self) -> impl Iterator<Item = &Path> {
        self.repos.iter().map(|(_, dir)| dir.path())
    }

    // where a set lives, given the repo it was created in. mirrors what set resolution does,
    // without depending on it, so tests can assert on the result of resolution.
    pub(crate) fn set_path_in(&self, repo: &str, set_name: &str) -> PathBuf {
        self.repo_root_for(&RepoName::from(repo)).join(set_name)
    }

    pub(crate) fn local_root(&self) -> &Path {
        self.local_root.path()
    }

    pub(crate) fn data_root(&self) -> &Path {
        self.data_root.path()
    }

    pub(crate) fn profile_path(&self) -> &Path {
        &self.profile_path
    }

    pub(crate) fn local_path(&self, path: &str) -> LocalFilePath {
        LocalFilePath::from(
            &self.profile().unwrap(),
            path.as_ref(),
            self.local_root.path(),
        )
        .unwrap()
    }

    pub(crate) fn cwd(&self) -> LocalFilePath {
        LocalFilePath::from(
            &self.profile().unwrap(),
            self.local_root.path(),
            self.local_root.path(),
        )
        .unwrap()
    }

    pub(crate) fn profile(&self) -> std::result::Result<MonjaProfile, MonjaProfileConfigError> {
        // we previously stored an instance of the profile
        // however, we changed it to reading a file to get coverage of the code paths
        let local_root = AbsolutePath::for_existing_path(self.local_root.path()).unwrap();
        let data_root = AbsolutePath::for_existing_path(self.data_root.path()).unwrap();

        // NOTE: MonjaProfile::from_config just gives an io::Error, but that's getting into'd into a MonjaProfileConfigError
        // which works fine for our case, but don't be misled!
        MonjaProfile::from_config(
            MonjaProfileConfig::load(&self.profile_path)?,
            local_root,
            data_root,
        )
        .map_err(MonjaProfileConfigError::from)
    }

    pub(crate) fn execution_options(&self) -> &ExecutionOptions {
        &self.opts
    }

    pub(crate) fn dryrun(&mut self, dry_run: bool) -> &mut Self {
        self.opts.dry_run = dry_run;

        self
    }

    pub(crate) fn configure_profile<P>(&self, config: P) -> &Self
    where
        P: FnOnce(MonjaProfileConfig) -> MonjaProfileConfig,
    {
        let profile_config = config(self.profile().unwrap().config);
        profile_config.save(&self.profile_path).unwrap();

        self
    }

    pub(crate) fn configure_set<P>(&self, set_name: SetName, config: P) -> &Self
    where
        P: FnOnce(SetConfig) -> SetConfig,
    {
        self.configure_set_in(&self.repos[0].0.clone(), set_name, config)
    }

    pub(crate) fn configure_set_in<P>(&self, repo: &RepoName, set_name: SetName, config: P) -> &Self
    where
        P: FnOnce(SetConfig) -> SetConfig,
    {
        let set_root = self.repo_root_for(repo).join(&set_name);
        let set_config = SetConfig::load(&set_root, &set_name).unwrap();
        let set_config = config(set_config);

        set_config.save(&set_root, &set_name).unwrap();

        self
    }

    // adding is handled by set_operation!
    pub(crate) fn rem_set(&self, set_name: SetName) -> &Self {
        let path = self.repo_root().join(set_name);
        fs::remove_dir_all(path).unwrap();

        self
    }
}

pub(crate) trait OperationHandler {
    fn dir(&mut self, path: &Path);
    fn remove_dir(&mut self, path: &Path);

    fn file(&mut self, path: &Path, contents: &str);
    fn remove_file(&mut self, path: &Path);

    fn finish(self);
}

pub(crate) struct Manipulation;
impl Manipulation {
    pub(crate) fn new(_: &Simulator) -> Self {
        Manipulation
    }
}
impl OperationHandler for Manipulation {
    fn dir(&mut self, path: &Path) {
        fs::create_dir_all(path).unwrap();
    }

    fn remove_dir(&mut self, path: &Path) {
        fs::remove_dir_all(path).unwrap();
    }

    fn file(&mut self, path: &Path, contents: &str) {
        fs::write(path, contents).unwrap();
    }

    fn remove_file(&mut self, path: &Path) {
        fs::remove_file(path).unwrap();
    }

    fn finish(self) {}
}

pub(crate) struct LocalValidation {
    local_root: PathBuf,
    repo_roots: Vec<PathBuf>,
    general_validation: GeneralValidation,
}
impl LocalValidation {
    pub fn new(sim: &Simulator) -> Self {
        LocalValidation {
            local_root: sim.local_root().to_path_buf(),
            repo_roots: sim.repo_roots().map(|p| p.to_path_buf()).collect(),
            general_validation: GeneralValidation::new(sim),
        }
    }
}
impl OperationHandler for LocalValidation {
    fn dir(&mut self, path: &Path) {
        self.general_validation.dir(path);
    }

    fn remove_dir(&mut self, path: &Path) {
        self.general_validation.remove_dir(path);
    }

    fn file(&mut self, path: &Path, expected_contents: &str) {
        self.general_validation.file(path, expected_contents);
    }

    fn remove_file(&mut self, path: &Path) {
        self.general_validation.remove_file(path);
    }

    fn finish(self) {
        let local_files: HashSet<PathBuf> = WalkDir::new(&self.local_root)
            .into_iter()
            .map(|e| e.unwrap())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .filter(|p| !monja::is_monja_special_file(p))
            .filter(|p| !self.repo_roots.iter().any(|root| p.starts_with(root)))
            .collect();

        expect_that!(local_files, container_eq(self.general_validation.files));
    }
}

pub(crate) struct SetValidation {
    set_root: PathBuf,
    general_validation: GeneralValidation,
}

impl SetValidation {
    pub fn new(sim: &Simulator, set_root: &Path) -> Self {
        SetValidation {
            set_root: set_root.to_path_buf(),
            general_validation: GeneralValidation::new(sim),
        }
    }
}

impl OperationHandler for SetValidation {
    fn dir(&mut self, path: &Path) {
        self.general_validation.dir(path);
    }

    fn remove_dir(&mut self, path: &Path) {
        self.general_validation.remove_dir(path);
    }

    fn file(&mut self, path: &Path, expected_contents: &str) {
        self.general_validation.file(path, expected_contents);
    }

    fn remove_file(&mut self, path: &Path) {
        self.general_validation.remove_file(path);
    }

    fn finish(self) {
        let repo_files: HashSet<PathBuf> = WalkDir::new(&self.set_root)
            .into_iter()
            .map(|e| e.unwrap())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.into_path())
            .filter(|p| !monja::is_monja_special_file(p))
            .collect();

        expect_that!(repo_files, container_eq(self.general_validation.files));
    }
}

struct GeneralValidation {
    pub files: HashSet<PathBuf>,
}

impl GeneralValidation {
    pub fn new(_: &Simulator) -> Self {
        GeneralValidation {
            files: HashSet::new(),
        }
    }
}

impl OperationHandler for GeneralValidation {
    fn dir(&mut self, path: &Path) {
        expect_that!(path.is_dir(), is_true());
        expect_that!(path.exists(), is_true());
    }

    fn remove_dir(&mut self, path: &Path) {
        expect_that!(path.exists(), is_false());
    }

    fn file(&mut self, path: &Path, expected_contents: &str) {
        match fs::read_to_string(path) {
            Ok(contents) => {
                expect_that!(contents, eq(expected_contents));
                expect_that!(self.files.insert(path.to_path_buf()), is_true());
            }
            Err(err) => {
                add_failure!("Error reading file: {}", err);
            }
        };
    }

    fn remove_file(&mut self, path: &Path) {
        expect_that!(path.exists(), is_false());
    }

    fn finish(self) {}
}

pub(crate) fn set_names<S, N>(names: N) -> Vec<SetName>
where
    S: AsRef<str>,
    N: AsRef<[S]>,
{
    names
        .as_ref()
        .iter()
        .map(|n| SetName(n.as_ref().into()))
        .collect()
}

// a previous version used lambda-based (nested) builders to do the same thing.
// while reasonably readable, it was also somewhat hard to read and get parenthesis matched up correctly
// though macros have horrible diagnostics when there's an issue, it overall works well!
#[allow(unused_macros)]
macro_rules! fs_operation {
    // largely follows https://danielkeep.github.io/tlborm/book/aeg-ook.html

    // these don't really need to be last, since the internal stuff won't match this.
    // so putting it at the top to stand out more
    // `RepoSet*` variants take an explicit repo name, for when a test has more than one.
    (RepoSetManipulation, $sim:expr, $repo:literal, $set:literal, $($tokens:tt)*) => {
        {
            let path = $sim.set_path_in($repo, $set);
            let mut handler = $crate::sim::Manipulation::new(&$sim);
            fs_operation!(@start (handler, path); ($($tokens)*));
        }
    };

    (RepoSetValidation, $sim:expr, $repo:literal, $set:literal, $($tokens:tt)*) => {
        {
            let path = $sim.set_path_in($repo, $set);
            let mut handler = $crate::sim::SetValidation::new(&$sim, &path);
            fs_operation!(@start (handler, path); ($($tokens)*));
        }
    };

    (SetManipulation, $sim:expr, $set:literal, $($tokens:tt)*) => {
        {
            let path = $sim.repo_root().join($set);
            let mut handler = $crate::sim::Manipulation::new(&$sim);
            fs_operation!(@start (handler, path); ($($tokens)*));
        }
    };

    (LocalManipulation, $sim:expr, $($tokens:tt)*) => {
        {
            let path = $sim.profile().unwrap().local_root.into_path_buf();
            let mut handler = $crate::sim::Manipulation::new(&$sim);
            fs_operation!(@start (handler, path); ($($tokens)*));
        }
    };

    (SetValidation, $sim:expr, $set:literal, $($tokens:tt)*) => {
        {
            let path = $sim.repo_root().join($set);
            let mut handler = $crate::sim::SetValidation::new(&$sim, &path);
            fs_operation!(@start (handler, path); ($($tokens)*));
        }
    };

    (LocalValidation, $sim:expr, $($tokens:tt)*) => {
        {
            let path = $sim.profile().unwrap().local_root.into_path_buf();
            let mut handler = $crate::sim::LocalValidation::new(&$sim);
            fs_operation!(@start (handler, path); ($($tokens)*));
        }
    };

    (@start ($handler:ident, $path_var:ident); $tokens:tt) => {
        $crate::sim::OperationHandler::dir(&mut $handler, &$path_var);
        fs_operation!(@general ($handler, $path_var); $tokens);
        $crate::sim::OperationHandler::finish($handler);
    };

    (@general ($handler:ident, $path_var:ident); (file $path:literal $contents:literal $($tail:tt)*)) => {
        {
            let path = $path_var.join($path);
            $crate::sim::OperationHandler::file(&mut $handler, &path, $contents);
        }

        fs_operation!(@general ($handler, $path_var); ($($tail)*));
    };

    (@general ($handler:ident, $path_var:ident); (remfile $path:literal $($tail:tt)*)) => {
        {
            let path = $path_var.join($path);
            $crate::sim::OperationHandler::remove_file(&mut $handler, &path);
        }

        fs_operation!(@general ($handler, $path_var); ($($tail)*));
    };

    (@general ($handler:ident, $path_var:ident); (remdir $path:literal $($tail:tt)*)) => {
        {
            let path = $path_var.join($path);
            $crate::sim::OperationHandler::remove_dir(&mut $handler, &path);
        }

        fs_operation!(@general ($handler, $path_var); ($($tail)*));
    };

    (@general ($handler:ident, $path_var:ident); (dir $path:literal $($tail:tt)*)) => {
        {
            let path = $path_var.join($path);
            $crate::sim::OperationHandler::dir(&mut $handler, &path);

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
        fs_operation!(@extract_inner $symbols; $depth; ($($buffer)* remfile $path); ($($tail)*));
    };
    (@extract_inner $symbols:tt; $depth:tt; ($($buffer:tt)*); (remdir $path:literal $($tail:tt)*)) => {
        fs_operation!(@extract_inner $symbols; $depth; ($($buffer)* remdir $path); ($($tail)*));
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
