use std::{
    ffi::OsStr,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::ExecutionOptions;

// keeping as io result because basically everything is io result
pub(crate) fn rsync(
    source: &Path,
    dest: &Path,
    files: impl Iterator<Item = PathBuf>,
    opts: &ExecutionOptions,
) -> std::io::Result<()> {
    // we use checksum mainly because, in integration tests, some files have same size and modified time
    // this could hypothetically happen in practice, so checksum is perhaps good.
    // note that file sizes still get compared before checksum, so most cases will still be fast.
    let mut args: Vec<&OsStr> = vec![
        "-a".as_ref(),
        "--files-from=-".as_ref(),
        "--checksum".as_ref(),
        "--mkpath".as_ref(),
    ];
    if opts.verbosity > 0 {
        args.push("-v".as_ref());
    }
    args.push(source.as_os_str());
    // append a /
    // works with mkpath to ensure the dir is properly created if needed
    let dest = dest.join("").into_os_string();
    args.push(&dest);

    let mut child = Command::new("rsync")
        .args(args)
        .stdin(Stdio::piped())
        .spawn()?;

    {
        let mut stdin = child.stdin.take().expect("Added above");
        for file in files {
            // avoiding the fallible conversion to string
            stdin.write_all(file.as_os_str().as_encoded_bytes())?;
            stdin.write_all(b"\n")?;
        }
        // dropping sends eof
    }

    let status = child.wait_with_output()?;
    if opts.verbosity > 0 {
        println!(
            "Finished rsync for '{}' with status {}",
            dest.display(),
            status.status
        );
    }

    match status.status.success() {
        true => Ok(()),
        false => Err(std::io::Error::other("Unsuccessful status code for rsync.")),
    }
}
