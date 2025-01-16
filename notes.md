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
    root = '~/'

    # will also do packages later. still mulling if to support yay/pacman only or not.
    # probably will support more, and will have .monja-profile contain a command format for adding
    # plus a way to remap package names

.monja-dir.toml
    # follows getfacl and setfacl conventions
    # since not meant to be run under sudo, will not be able to set owner or group. could change later tho
    [perms.default]
    # default perms for the current folder
    # also will be considered the perms, by default, for everything in the folder
    [perms.files]
    # files and directories separated out for readability
    [perms.directories]
    object = [
        "user::r-x",
    ]
    # todo: will probably go with exacl crate, which may effect the serialization

    # useful for broad directories like /.config
    noclean = true


Special local files
    ~/.monja-profile.toml
        monjadir = "~/monja"
        profile = "~/monja/profile1.toml" # an alternative to all of the below, useful for checking in profiles
        sets = [
            "set2",
            "set1",
        ]

        [default-set]
        object = "set1"

    ~/.monja-index.toml
        will map every file to an associated set
        TODO: not meant to be edited, so might be better to just expose viewing this as a command
            this only really becomes useful if we can avoid regenerating it fully for each push.
                since local file edits are done without running cli, we don't get this.
                unless we add shell hooks, which may indeed be useful but not a v1 feature.
                so probably wont have this file initially.
                and also sqlite is likely better anyway.
                TODO: keep these design notes if we delete this from this file.
        when the file is modified and needs to be synced back to monjadir, it uses this set
        if a file is contained in multiple sets, the latest set it used
        overrides can be added via [default-set]
            this can be done for files, but it's perhaps more preferred to add the file to the appropriate later set
            this is most useful for directories. we can add a file from a "default set", and any future changes get applied to a different set

    ~/.monjaignore
        ignorefile that keeps stuff from being pushed to monja and keeps stuff from being cleaned by monja
    
command-line interface
    dry run of course

    push -- have monja scan local files for changes and update monjadir
    pull -- have monja modify local files
        clean -- if a directory is in monja and there are local objects not in monja, delete them
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

