get mvp up and running, since want practical usage to drive design

Monja file structure /
    profile1.toml
    profile2.toml
    monjasets/
        set1/
        set2/
            `<files>`
            `<directories>`
                ...
                .monja-dir.toml
            .monja-set.toml
            .monja-dir.toml


.monja-set.toml
    # if a set, for instance, only contains ~/.config/foo, can avoid annoying to navigate nesting
    # root = '.config/foo'
    # noting that this root is relative to a profile's root, hence a relative path and no `~/`
    root = ''

    # will also do packages later.
    # will probably allow multiple sets of packages -- one for each source: pacman, aur, snap, etc
    # will also have a command template
    # would also perhaps need an ordering mechanism -- to start with pacman, then yay, etc.
    #   also may imply some sort of bootstrapping commands


.monja-dir.toml
    # would set owner and group to current user
    # would generally be a noop for git

    # perms not in v1
    # follows getfacl and setfacl conventions
    [perms.default]
    # default perms for the current folder. is recursive
    [perms.files]
    # files and directories separated out for readability
    [perms.directories]
    object = [
        "user::r-x",
    ]
    # todo: will probably go with exacl crate, which may effect the serialization

    # useful for broad directories like /.config
    # cleaning a later feature
    clean_on_pull = true


Special local files
    ~/.monja-profile.toml
        monjadir = "~/monja"
        profile = "~/monja/profile1.toml" # an alternative to all of the below, useful for checking in profiles
        sets = [
            "set2",
            "set1",
        ]
        new_file_set = "set1" # or last by default

    ~/.monja-index.toml
        will map every local file to an associated set
        gets refreshed each pull
        needed in case monjadir changes what set will be used when pushing back
        when a file is modified and needs to be synced back to monjadir, it uses this set

    ~/.monjaignore
        ignorefile that keeps stuff from being pushed to monja and keeps stuff from being cleaned by monja
    
command-line interface
    dry run of course

    note: prob not doing any cleaning to start

    push -- have monja scan local files for changes and update monjadir
        does not add new files
    pull -- have monja modify local files
        clean (comes later) -- if a directory is in monjadir and there are local objects not in monjadir, delete them
            clean by default; prompt with list by default; allow force; allow noclean
            TODO: might split off into another command. if someone wants to pull a change without having pushed prior, dont want to be annoying
                on the other hand, it would be confusing if making  removing from somewhere else, pulling, and wondering why the removed thing is still around
                maybe a quickpull that  does no clean?
        since will be run under the assumption all files have right owner and group, also need a mode that can be invoked under sudo (explicit or implicit?) to force ownership
            will failfast instead of partial syncing
            will support a way to skip
            and will have a separate command to fix ownership

    init -- create profile and monjadir
    addset
        also add an interactive mode, where unselected things get put into a .monjaignore
    setdefaultperms path
        will also regenerate perms config against new default
    merge
        do a 3 way merge between local file, associated set, and a specified target set
        useful for applying local file parts to a target set
        also useful for adding parts of the file to a later set that doesnt currently have the file, making it the new associated set

design thoughts
    dont see the tool being particularly complex, dont go strong on ports and adapters, avoid abstracting too much
    stick with a single package, of course
        though the urge to have cli and lib in separate packages is strong.
        will at least use separate modules for them
    focus on integration testing instead of unit testing
        so filesystem operations part of testing
        if we get to packages, will abstract that one out, since dont want to add and remove packages when automated testing
        while we'll separate out lib and cli into separate modules, drive cli through integration testing just to get coverage on it
            since ease of use is a key requirement, we'll auto command lines in tests, versus some complicated command builder
            will attempt to be light on config generation, preferring to put raw configs in the tests themselves, but not afraid to get fancier, since readability is key.
    and the point of keeping cli and lib in separate modules? flexibility and ease of reasoning
        one key point being that package management will probably get unit tested -- meaning avoiding integration testing
            though, since we'll probably allow user-provided command format, integration testing is still doable. still thinking about it, and package management hasnt even been designed yet.
        and should the lib get more complex over time, we can add unit tests without first having to factor things out of cli. though may need to factor out fs stuff -- such is the gamble.

rsync
    idea is that rsync is a better way to batch copy files
    also want to support a more manual approach, so implement both

    Can use rsync like…
    rsync -avR /foo/./bar/baz.c /tmp/
    Where -R is relative
    And the middle dot cuts off the foo
    Which should work since the managed files will all be relative to a folder

    Also, rsync supports multiple src files and single destination (directory, in our case)

    For syncing back, we group by sets, since we can only specify one destination