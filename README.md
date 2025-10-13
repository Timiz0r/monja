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

## Usage
TODO