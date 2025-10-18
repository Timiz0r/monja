# Monja
Monjayaki ( もんじゃ焼き : /moɴd͡ʑa jaːki/ ), often shortened to monja, is a delicious Japanese food that I can't really describe properly.
I just like naming projects after my favorite foods 🤷.

As far as this project is concerned, Monja is a very simple to use and easy to reason about multi-machine dotfiles manager.
Files are stored in a `sets` found in a `repo`,
and a portion (or all) of these sets can be chosen to be synchronized locally.
If a file is found in multiple sets, then the latest set's file wins.

There is no templating engine. Instead, split files across sets in some appropriate way, and,
if config duplication becomes a concern,
use the right configurations to source/import/include/configure the right parts for the right machine,
using the typical methods for each tool.

## Dependencies
* rsync
  * We use `rsync` because it's an already existing, well-know, quality tool that has great performance and reliability.
    Why invent our own wheel when the perfect one already exists?
* fzf
  * Used for interactively adding files to the monja repo.
* bat
  * Used for file previews in `fzf`

## Usage
Quick note: any of the below commands that touch files support the `--dryrun` flag
to view operations without performing them.

### Initialization
To get started, use `monja init` to create a default profile and repo.

The profile is responsible for deciding what sets will be pulled from the repo.
You can view the profile with `cat $(monja profile)`

A default set named after `hostname` will be created.
You can head to the repo to view this empty set with `monja repodir | cd`.

A default .monjaignore will also be placed in `$HOME`.
By default, it filters out most directories from `$HOME` but allows `.config`.

### Adding files to repo
Files can be added to the default set with `monja put -i`.
This starts `fzf` with the list of files in cwd -- except those already in the set.
You can also disregard cwd and pick from any file in `$HOME` (sans ignored) by adding the `--nocwd` flag.

You can create a new set with `monja newset --set mycoolset -i`.
Again, this will provide `fzf` with a list of files in cwd -- every single one (sans ignored).
The `--nocwd` flag is usable here, as well.
This command will create a new set, copy the files to it, and modify the profile to use the new set.
If all files in the set have a common prefix, the set will be configured with a `shortcut` to reduce folder nesting.

Also note that `monja newset` can also take files via `-- <file 1> <file 2> ...` or newline-delimited stdin.
In fact, all three methods of specifying files can be combined.

### `git init`
You'll probably want to turn your monja repo into a git repo.
You can navigate to it quickly with `monja repodir | cd`.

### Pushing to the repo
To put local changes into the repo, simply run `monja push`.
Any file that was previously pulled (or `monja newset`ed) will be copied to the repo, into the set from whence it came.

**Important:** `monja push` may fail depending on modifications done to the repo.
`monja push` keeps a local index that maps files to a corresponding set.
If these files are removed or otherwise don't match up, `monja push` will fail.
As such, it is recommended to `monja push` before `git pull`ing in the repo.
Still, there are ways to recover from this issue if it happens.

#### Recovering from broken `monja push`
You may get errors like these:
* > There are local files whose corresponding sets are missing.
* > There are local files missing from expected sets.

To recover, use `monja put --update-index --set <target set> -- <files>`.
This command also supports `-i` and line-delimited stdin -- the same as `monja newset`.

Once the affected files have been `monja put` back, you can `monja push` again.

### Pulling from the repo
**Important:** `monja pull` will happily overwrite local files without warning, so be sure to `monja push` first.

To pull from the repo, simply run `monja pull`.
It copies the files from the sets targeted by the profile and copies it locally.
If the same file is in multiple sets, the latest set's file wins.

### Cleaning
There are two kinds of clean: index and full.

The default index clean can be invoked with `monja clean`. It will look at the diff between the last two `monja pull`s
and only remove the files that were in the older pull but not the newer pull.

By adding the `--full` flag, the full local state will be compared to the repo,
and any file not in the repo (but local) will be removed.

The clean command will list the files to be cleaned and ask for confirmation.
You can also use the `--dryrun` flag to see the output of operations like `monja clean` without actually performing them.