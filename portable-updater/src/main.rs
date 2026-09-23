#![deny(clippy::all)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use anyhow::{Context, Result, anyhow, bail};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

struct Options {
    parent_pid: u32,
    archive: PathBuf,
    extractor: PathBuf,
    application: PathBuf,
}

fn parse_options(arguments: impl IntoIterator<Item = String>) -> Result<Options> {
    let mut parent_pid = None;
    let mut archive = None;
    let mut extractor = None;
    let mut application = None;
    let mut values = arguments.into_iter();
    while let Some(flag) = values.next() {
        let value = values
            .next()
            .ok_or_else(|| anyhow!("missing value for {flag}"))?;
        if value.contains(['\0', '\r', '\n']) {
            bail!("portable update argument contains a control character");
        }
        let slot = match flag.as_str() {
            "--parent-pid" => &mut parent_pid,
            "--archive" => &mut archive,
            "--extractor" => &mut extractor,
            "--application" => &mut application,
            _ => bail!("unsupported portable update argument: {flag}"),
        };
        if slot.replace(value).is_some() {
            bail!("duplicate portable update argument: {flag}");
        }
    }
    let parent_pid = parent_pid
        .ok_or_else(|| anyhow!("missing Desktop process ID"))?
        .parse::<u32>()
        .context("invalid Desktop process ID")?;
    if parent_pid == 0 {
        bail!("invalid Desktop process ID");
    }
    Ok(Options {
        parent_pid,
        archive: PathBuf::from(archive.ok_or_else(|| anyhow!("missing archive"))?),
        extractor: PathBuf::from(extractor.ok_or_else(|| anyhow!("missing extractor"))?),
        application: PathBuf::from(application.ok_or_else(|| anyhow!("missing application"))?),
    })
}

fn canonical_file(path: &Path, name: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!("{name} must be an absolute path");
    }
    let canonical = fs::canonicalize(path).with_context(|| format!("locate {name}"))?;
    if !fs::metadata(&canonical)?.is_file() {
        bail!("{name} is not a regular file");
    }
    Ok(canonical)
}

fn validate_paths(options: &Options) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let archive = canonical_file(&options.archive, "archive")?;
    let extractor = canonical_file(&options.extractor, "extractor")?;
    let application = canonical_file(&options.application, "application")?;
    let archive_is_7z = archive
        .extension()
        .and_then(|part| part.to_str())
        .is_some_and(|part| part.eq_ignore_ascii_case("7z"));
    let is_named = |path: &Path, expected: &str| {
        path.file_name()
            .and_then(|part| part.to_str())
            .is_some_and(|part| part.eq_ignore_ascii_case(expected))
    };
    if !archive_is_7z
        || !is_named(&extractor, "7za.exe")
        || !is_named(&application, "KokoroBox.exe")
    {
        bail!("invalid portable update paths");
    }
    if archive.parent() != extractor.parent() {
        bail!("archive and extractor must share a directory");
    }
    // Validate resolved files, but pass the original absolute paths onward:
    // Windows canonical paths may carry a verbatim prefix unsupported by 7za.
    Ok((
        options.archive.clone(),
        options.extractor.clone(),
        options.application.clone(),
    ))
}

#[cfg(target_os = "windows")]
fn wait_for_desktop(parent_pid: u32) -> Result<()> {
    use windows::Win32::{
        Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };

    let handle = match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, parent_pid) } {
        Ok(handle) => handle,
        Err(error) if error.code() == ERROR_INVALID_PARAMETER.into() => return Ok(()),
        Err(error) => return Err(error).context("open Desktop process for wait"),
    };
    let result = unsafe { WaitForSingleObject(handle, 120_000) };
    let _ = unsafe { CloseHandle(handle) };
    match result {
        WAIT_OBJECT_0 => Ok(()),
        WAIT_TIMEOUT => bail!("Desktop did not exit before portable update"),
        _ => bail!("failed to wait for Desktop process"),
    }
}

#[cfg(not(target_os = "windows"))]
fn wait_for_desktop(_parent_pid: u32) -> Result<()> {
    bail!("portable updater is only available on Windows")
}

fn run(options: Options) -> Result<()> {
    let (archive, extractor, application) = validate_paths(&options)?;
    wait_for_desktop(options.parent_pid)?;
    let target = application
        .parent()
        .ok_or_else(|| anyhow!("invalid application path"))?;
    let output = Command::new(extractor)
        .arg("x")
        .arg(format!("-o{}", target.display()))
        .arg("-y")
        .arg(archive)
        .current_dir(target)
        .output()
        .context("start portable update extraction")?;
    if !output.status.success() {
        bail!(
            "extract portable update failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Command::new(application)
        .spawn()
        .context("restart KokoroBox")?;
    Ok(())
}

fn main() {
    if env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--help")) {
        println!(
            "KokoroBox portable updater: --parent-pid PID --archive FILE --extractor FILE --application FILE"
        );
        return;
    }
    let result = parse_options(env::args().skip(1)).and_then(run);
    if let Err(error) = result {
        eprintln!("kokorobox-portable-updater: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_duplicate_and_invalid_arguments() {
        assert!(parse_options(Vec::<String>::new()).is_err());
        assert!(parse_options(["--parent-pid", "0"].map(String::from)).is_err());
        assert!(parse_options(["--unknown", "x"].map(String::from)).is_err());
        assert!(
            parse_options(["--parent-pid", "1", "--parent-pid", "2"].map(String::from)).is_err()
        );
    }

    #[test]
    fn accepts_the_bounded_update_contract() {
        let options = parse_options(
            [
                "--parent-pid",
                "42",
                "--archive",
                "C:\\Data\\update.7z",
                "--extractor",
                "C:\\Data\\7za.exe",
                "--application",
                "C:\\App\\KokoroBox.exe",
            ]
            .map(String::from),
        )
        .unwrap();
        assert_eq!(options.parent_pid, 42);
    }
}
