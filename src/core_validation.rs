//! Bounded, fixed-argument Mihomo validation; no arbitrary command execution.
use anyhow::{Context, Result, bail};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
const MAX_OUTPUT: usize = 256 * 1024;
#[derive(Debug, Clone)]
pub struct CoreValidationOptions {
    pub executable: String,
    pub config_path: String,
    pub work_dir: String,
    pub safe_paths: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct CoreValidationResult {
    pub outcome: String,
    pub output: String,
}
fn absolute(path: &str) -> Result<PathBuf> {
    if !Path::new(path).is_absolute() || path.contains('\0') {
        bail!("validation paths must be absolute");
    }
    std::fs::canonicalize(path).context("resolve validation path")
}
pub fn validate_core_profile(options: &CoreValidationOptions) -> Result<CoreValidationResult> {
    let executable = crate::core_process::core_path(&options.executable)?;
    let config = absolute(&options.config_path)?;
    let work = absolute(&options.work_dir)?;
    if !config.is_file() || !work.is_dir() {
        bail!("invalid validation config or working directory");
    }
    if options.safe_paths.len() > 64 {
        bail!("too many trusted paths");
    }
    let safe = options
        .safe_paths
        .iter()
        .map(|path| {
            let path = absolute(path)?;
            let text = path.to_string_lossy().into_owned();
            if text.contains(if cfg!(windows) { ';' } else { ':' }) {
                bail!("trusted path contains environment delimiter");
            }
            Ok(text)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut command = Command::new(executable);
    command
        .args(["-t", "-f"])
        .arg(config)
        .arg("-d")
        .arg(&work)
        .current_dir(work)
        .env(
            "SAFE_PATHS",
            safe.join(if cfg!(windows) { ";" } else { ":" }),
        )
        .env("CLASH_CONFIG_STRING", "")
        .env("CLASH_CONFIG_FILE", "")
        .env("CLASH_HOME_DIR", "")
        .env("CLASH_POST_UP", "")
        .env("CLASH_POST_DOWN", "");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    run(command, Duration::from_secs(10))
}
fn run(mut command: Command, timeout: Duration) -> Result<CoreValidationResult> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().context("start core validation")?;
    let (sender, receiver) = mpsc::channel();
    for mut pipe in [
        Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>,
        Box::new(child.stderr.take().unwrap()),
    ] {
        let sender = sender.clone();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut chunk = [0; 8192];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) => {
                        let _ = sender.send(Some(bytes));
                        return;
                    }
                    Ok(n) if bytes.len() + n <= MAX_OUTPUT => bytes.extend_from_slice(&chunk[..n]),
                    _ => {
                        let _ = sender.send(None);
                        return;
                    }
                }
            }
        });
    }
    drop(sender);
    let deadline = Instant::now() + timeout;
    let mut output = Vec::new();
    let mut finished = 0;
    let result = 'wait: loop {
        loop {
            match receiver.try_recv() {
                Ok(Some(bytes)) => {
                    output.extend(bytes);
                    finished += 1;
                }
                Ok(None) => {
                    break 'wait CoreValidationResult {
                        outcome: "output-limit".into(),
                        output: String::from_utf8_lossy(&output).into_owned(),
                    };
                }
                Err(_) => break,
            }
        }
        match child.try_wait() {
            Ok(Some(status)) if finished == 2 => {
                break CoreValidationResult {
                    outcome: if status.success() { "valid" } else { "invalid" }.into(),
                    output: String::from_utf8_lossy(&output).into_owned(),
                };
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
            _ => {}
        }
        if Instant::now() >= deadline {
            break CoreValidationResult {
                outcome: "timeout".into(),
                output: String::from_utf8_lossy(&output).into_owned(),
            };
        }
        thread::sleep(Duration::from_millis(10));
    };
    let _ = child.kill();
    let _ = child.wait();
    Ok(result)
}
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn validation_is_bounded_and_preserves_failure_output() {
        let mut invalid = Command::new("/bin/sh");
        invalid.args(["-c", "echo level=error invalid-profile >&2; exit 1"]);
        let result = run(invalid, Duration::from_secs(1)).unwrap();
        assert_eq!(result.outcome, "invalid");
        assert!(result.output.contains("invalid-profile"));
        let mut timeout = Command::new("/bin/sleep");
        timeout.arg("30");
        assert_eq!(
            run(timeout, Duration::from_millis(30)).unwrap().outcome,
            "timeout"
        );
        let mut noisy = Command::new("/usr/bin/yes");
        noisy.arg("diagnostic");
        assert_eq!(
            run(noisy, Duration::from_secs(1)).unwrap().outcome,
            "output-limit"
        );
    }
    #[test]
    fn arbitrary_programs_are_rejected() {
        assert!(
            validate_core_profile(&CoreValidationOptions {
                executable: "/bin/sh".into(),
                config_path: "/tmp/config".into(),
                work_dir: "/tmp".into(),
                safe_paths: vec![]
            })
            .is_err()
        );
    }
}
