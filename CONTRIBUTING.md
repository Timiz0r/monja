## Overview
While mainly a personal project to sync dotfiles the way I would like to, this project is fully open to contribution!
This document is very WIP and likely incomplete, so apologies if something important is missing!

### Purpose
Monja is intended to be an extemely simple way to synchronize dotfiles across multiple machines,
where each may get a different *set* of files.

We don't do templating.
Instead, each *profile* can choose which *sets* of directories it wants to use.
If two machines want a different set of neovim keybinds, for instance, then they would go in different *sets*,
and the machine would choose which set to use.

Additionally, it's up to the user to figure out the best way to handle complicated "merging" scenarios, such as via...
* Shell script logic
* Conditionally sourcing shell scripts
* Putting some files in a common set, then other files in machine-specific sets

### Glossary

| Term       | Description                                                                                                      |
| ---------- | ---------------------------------------------------------------------------------------------------------------- |
| Repo       | We *pull* configs from here to *local*. This is typically also a git repo.                                       |
| Sets       | Directory that exists in the root of the *repo*. They contain the files that can be *pulled* to *local*.         |
| Set config | A `.monja-set.toml` file found in the root of each *set's* directory.                                            |
| Local      | Typically `~/`. We *push* files from here to the *repo*.                                                         |
| Profile    | Defined by a file in `~/.config/monja/monja-profile.toml`. Mainly defines which *sets* are used from the *repo*. |
| Push       | The operation that puts files that have been updated back into the *repo*.                                       |
| Pull       | The operation that copies files from the *repo* to local.                                                        |

## New features and functionality
Be sure to open an issue or find an existing one to discuss beforehand,
just to avoid potentially wasting time on something that won't make it in.

## Bugs and improvements
These don't necessarily need an issue and can just be worked on whenever -- though issues are certainly handy.
Of course, keep in mind that creating an issue may be adviseable for large-scoped changes -- as per above, to avoid time wastage.

## Features on the todo list
* Packages
  * Key feature. Support any package manager.
* Git hook to warn against `git pull`ing when a `monja push` hasn't been done recently.
  * Key feature
* End-to-end tests
  * We'd mainly test that we can invoke the executable correctly, not necessarily every scenario of each subcommand.
* Permissions
  * Not too important. Git tracks execution bit, owner is probably all the same, and rw permission are also probably all the same.
* Diff/merge between local and sets
  * QoL
* Storing profiles in repo
  * `monja-profile.toml` files can be put in the root of the repo, without repo-dir specified,
    and they can be referenced from the usual `monja-dir.toml` in `$HOME/.config/monja`

The thoughts on these should eventually make it to an issue somewhere.

## Design
Probably the most incomplete section of them all!

The project *roughly* follows the ports-and-adapters pattern (aka is *roughly* hexagonally architectured).
Currently, there is only one application, signified by the `monja` crate (`lib.rs`).
Also see [this handy but insightful video on ports-and-adapters](https://www.youtube.com/watch?v=EZ05e7EMOLM) if you're interested!
Perhaps the biggest consequence is in automated testing, which we'll get to later.

The publicly exposed code found in `monja` are the driver ports (concrete types/functions that call into the application).
Each major operation, or sets of operations, are in their own module under `monja::operation`, which all gets reexported.
`lib.rs` contains bits that are both public and reasonably common.

Integration tests and the main function are the driver adapters that drive these ports.

There are no driver ports (public `traits` that call out from the application to external dependencies).
The only external dependency we deal with is the filesystem, and it has been decided to not abstract around it.

Otherwise, the internals of `monja` are relatively flexible to be redesigned, refactored, rewritten, etc.

### rsync
We use rsync because it's an already existing, well-know, quality tool that has great performance and reliability.
Why invent our own wheel when the perfect one already exists?

As such, we haven't done much abstraction around rsync, aside from the function of the same name.
Implementing our own copying isn't out of the question, though, should someone do the work of implementing it!

### Automated testing
We generally wouldn't use "Rust-style" unit tests, since ports-and-adapters prefer us verifying our application at its boundaries
(in order to get coverage over all of the application, and cover it similarly to how it will be used in pratice).
These kinds of tests would be done through "Rust-style" integration test. But, again, we don't have these kinds of tests.
Still, "Rust-style" unit tests (within a `tests` sub module) aren't banned or anything and can be used where useful,
as long as they don't cause any tight-coupling that makes the code hard to change.

Instead of "Rust-style" unit tests or even "ports-and-adapters-style unit tests", we just have integration tests,
since we don't even attempt to abstract around the filesystem.
In general, we want these tests to be written as close to how the application gets used in practice.
Naturally, this includes every edge case we can think of, just without coupling to the internals.

## Style
Mostly nothing of note yet. Note that I (Timiz0r, project owner) and relatively new to Rust,
so I'm very open to all sorts of improvements.

### Encapsulation
In general, for both internal and public structs, prefer no encapsulation --
aka expose fields publicly and don't use getters and setters.
By not encapsulating, we allow partial-borrows and partial-moves, plus the code quality and performance benefits of them.

Choosing to encapsulate mainly depends on the usual considerations:
* Importance and difficulty of maintaining a public interface
* Importance of maintaining consistency of an instance of the struct
* Importance of maintaing invariants of a field
  * Though, for this case, a new type can be created to maintain these
* Whether we want to pass around a mutated instance or not

This project doesn't *really* have a public interface (only used by tests and main),
so relying on software design, refactoring, and automated testing is sufficient, especially for internals.

Furthermore, most types we return aren't passed around and only get mutated in the function
they're returned to and consumed in, which makes them easier to understand.
This consideration has the additional benefit of making moves easier.

### Allocations
Performance isn't a major concern. Still, prefer to allocate and clone as little as possible.
This can additionally include things like...
* Prefering iterators over allocating new collections
* Allocating collections with a capacity
* Allocating at the caller, instead of cloning in the callee

### Turbofish
Prefer specifying the variable's type, even if it means an intermediate variable.
Somewhat lacking example:
```rs
// good:
let coll: Vec<Thing> = data.iter().filter(|i| i.is_cool()).collect();

// bad:
let c = data.iter().filter(|i| i.is_cool()).collect::<Vec<Thing>>();
```

### Helper functions and types
If a helper function or type is reasonably specific to the function it wants to help and is small enough,
put them in the function itself.

Additionally, prefer putting them at the bottom of the function, even if it means being forced to use `return`.
If helper functions are quite small, they can instead go at the bottom of the block they're in.

If a lot of data needs to be captured, using a lambda is also viable, keeping in mind the other alternatives.