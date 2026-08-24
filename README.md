# Monja
Monjayaki ( もんじゃ焼き : /moɴd͡ʑa jaːki/ ), often shortened to monja, is a delicious Japanese food
that I can't really describe properly.
I just like naming projects after my favorite foods 🤷.

As far as this project is concerned,
Monja is a very simple to use and easy to reason about multi-machine dotfiles manager.
Files are stored in `sets` found in one or more `repos`,
and a portion (or all) of these sets can be chosen to be synchronized locally.
If a file is found in multiple sets, then the latest set's file wins.

There is no templating engine. Instead, split files across sets in some appropriate way,
and, if config duplication becomes a concern,
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

## Installation
For now, this isn't uploaded anywhere. To install, checkout the repo and call `cargo install --path .` from the root.

### Shell completions
`monja` supports dynamic shell completions (including set names, read live from the profile/repo) via
[`clap_complete`](https://docs.rs/clap_complete). Source `monja completions` from your shell's startup file,
e.g. for fish: `echo 'monja completions | source' >> ~/.config/fish/config.fish`.

## Usage
Quick note: any of the below commands that touch files support the `--dry-run` flag
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
Files can be added to the default set with `monja file put -i`.
This starts `fzf` with the list of files in cwd -- except those already in the set.
You can also disregard cwd and pick from any file in `$HOME` (sans ignored) by adding the `--nocwd` flag.

You can create a new set with `monja newset --set mycoolset -i`.
Again, this will provide `fzf` with a list of files in cwd -- every single one (sans ignored).
The `--nocwd` flag is usable here, as well.
This command will create a new set, copy the files to it, and modify the profile to use the new set.
If all files in the set have a common prefix, the set will be configured with a `shortcut` to reduce folder nesting.
If the profile has multiple repos, add `--repo <repo>` (or configure a `default-repo`) to say
where the set should be created.

Also note that `monja newset` can also take files via `-- <file 1> <file 2> ...` or newline-delimited stdin.
In fact, all three methods of specifying files can be combined.

### `git init`
You'll probably want to turn your monja repo into a git repo.
You can navigate to it quickly with `monja repodir | cd`.

### Using multiple repos
A profile can draw sets from any number of repos -- handy when, say, work dotfiles live in one
repo and personal ones in another. This only widens the pool of sets the profile can target;
everything else works exactly as it does with one repo.

```toml
# these are layered on top of each other. if a file is in multiple sets, the later one wins.
# sets from different repos are layered together, purely in this order.
target-sets = [
    'common',
    'workstation',
]

# which repo `monja newset` and `monja repodir` act on.
# only needed when there's more than one repo.
default-repo = 'personal'

# each path can be absolute or relative to $HOME
[repos]
personal = '.local/share/monja/repos/personal'
work = '/srv/work-dotfiles'
```

The order of `[repos]` means nothing -- precedence comes solely from `target-sets`. Because of
that, a set name may only exist in one repo. If a name appears in several and the profile
actually uses it, monja errors out rather than guessing:

* If the duplicated name is in `target-sets`, commands like `monja file pull` fail.
* If it's passed explicitly (`--set`, `--from`, `--to`), that command fails.
* If it's never used, it's ignored, so an unrelated collision won't block you.

To fix a collision, rename the set in all but one of the repos.

Commands that don't reference an existing set have to be told which repo to act on:

```sh
monja newset --repo work --set worklaptop -i
monja repodir --repo work | cd
```

Both fall back to `default-repo`, and when only one repo is configured neither `--repo` nor
`default-repo` is needed at all.

The older single-repo form is still supported and behaves as one repo named `default`:

```toml
repo-dir = '.local/share/monja/repos/default'
```

Setting both `repo-dir` and `[repos]` is an error.

### Pushing to the repo
To put local changes into the repo, simply run `monja file push`.
Any file that was previously pulled (or `monja newset`ed) will be copied to the repo, into the set from whence it came.

**Important:** `monja file push` may fail depending on modifications done to the repo.
`monja file push` keeps a local index that maps files to a corresponding set.
If these files are removed or otherwise don't match up, `monja file push` will fail.
As such, it is recommended to `monja file push` before `git pull`ing in the repo.
Still, there are ways to recover from this issue if it happens.

#### Recovering from broken `monja file push`
You may get errors like these:
* > There are local files whose corresponding sets are missing.
* > There are local files missing from expected sets.

To recover, use `monja file put --set <target set> -- <files>`.
This command also supports `-i` and line-delimited stdin -- the same as `monja newset`.

Once the affected files have been `monja file put` back, you can `monja file push` again.

### Pulling from the repo
**Important:** `monja file pull` will happily overwrite local files without warning,
so be sure to `monja file push` first.

To pull from the repo, simply run `monja file pull`.
It copies the files from the sets targeted by the profile and copies it locally.
If the same file is in multiple sets, the latest set's file wins.

### Cleaning
There are two kinds of clean: index and full.

The default index clean can be invoked with `monja file clean`.
It will look at the diff between the last two `monja file pull`s
and only remove the files that were in the older pull but not the newer pull.

By adding the `--full` flag, the full local state will be compared to the repo,
and any file not in the repo (but local) will be removed.

The clean command will list the files to be cleaned and ask for confirmation.
You can also use the `--dry-run` flag to see the output of operations like `monja file clean`
without actually performing them.

### Moving files between sets
`monja file transfer --from <set> --to <set> -- <files>` moves already-tracked files from one set to another.
Like `monja file put`, it supports `-i` and line-delimited stdin for specifying files.

### Changing a set's shortcut
If all files in a set share a common prefix, that prefix can be configured as the set's `shortcut`,
so the set's files can be stored without repeating that prefix (this happens automatically for `monja newset`,
as noted above). To change a set's shortcut after the fact, use `monja file setshortcut --set <set> <path>`,
where `<path>` is the new shortcut, relative to `$HOME`. This restructures the files already in the set to match.

### Checking file status
`monja file status [location]` reports on the local files under `location` (or everywhere, if omitted):
which are untracked, which belong to sets missing from the repo, which are missing from the sets they were pulled
from, which are ready to be pushed, and which were removed from the repo since the last pull. Each of these
categories can be viewed in isolation with a corresponding flag (e.g. `--untracked`); see `monja file status --help`.

### Packages
In addition to files, a set can declare a list of packages it wants -- just plain package names,
with no version pinning or other metadata. Like files, a package declared by any of a profile's
targeted sets is part of that profile's effective package list. Unlike files, there's no
per-package "content" to have the latest set win over, so merging across sets is just a
deduplicated union.

Add packages to a set with `monja package add --set <set> <names...>`,
and remove them with `monja package remove --set <set> <names...>`.
These edit the `packages` list in that set's `.monja-set.toml` directly --
there's no local reflection of a package to push/pull the way there is for files.

`monja package list` shows the packages declared by each of the profile's targeted sets,
as well as the merged (effective) list.

`monja package install` installs the merged (effective) package list using the local machine's
package manager, by running a command you configure in `monja-profile.toml`:

```toml
[packages]
install-command = "sudo pacman -S {packages}"
# optional; defaults to a single space
install-delimiter = " "

# optional; translates a monja package name to whatever this machine's package manager
# actually calls it, only when building the install command (`monja package list` always
# shows the canonical name stored in `.monja-set.toml`)
[packages.aliases]
neovim = "neovim-git"
ripgrep = "rg"
```

The `{packages}` token gets replaced with the effective package list (aliases applied), joined by
`install-delimiter`, and the whole thing is run through `sh -c`, so the command can use ordinary
shell syntax (quoting, `&&`, `sudo`, etc.). `monja package install` prints the command before
running it and asks for confirmation (skippable with `-y`/`--yes`, same as other commands);
`--dry-run` builds and prints the command without running it. If no `install-command` is
configured, `monja package install` just reports the effective package list.