use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

const MAX_NAMES: usize = 32;
const MAX_ADDITIONAL_PATHS: usize = 64;

#[derive(Debug, Clone, Default)]
pub struct ExecutableSearchOptions {
    pub names: Vec<String>,
    pub additional_paths: Vec<String>,
    pub match_name_prefixes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableCandidate {
    pub path: String,
    pub canonical_path: String,
    pub name: String,
}

fn validate_options(options: &ExecutableSearchOptions) -> Result<()> {
    if options.names.is_empty() || options.names.len() > MAX_NAMES {
        return Err(anyhow!(
            "executable discovery requires between 1 and {MAX_NAMES} names"
        ));
    }
    if options.additional_paths.len() > MAX_ADDITIONAL_PATHS {
        return Err(anyhow!(
            "executable discovery accepts at most {MAX_ADDITIONAL_PATHS} additional paths"
        ));
    }

    for name in &options.names {
        if name.is_empty()
            || name.len() > 128
            || name == "."
            || name == ".."
            || name.contains(['/', '\\', '\0'])
        {
            return Err(anyhow!(
                "executable discovery names must be safe file basenames"
            ));
        }
    }
    for path in &options.additional_paths {
        if path.contains('\0') || !Path::new(path).is_absolute() {
            return Err(anyhow!(
                "executable discovery additional paths must be absolute directories"
            ));
        }
    }
    Ok(())
}

fn standard_search_directories() -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = env::var_os("PATH")
        .map(|value| {
            env::split_paths(&value)
                .filter(|path| path.is_absolute())
                .collect()
        })
        .unwrap_or_default();

    #[cfg(not(target_os = "windows"))]
    directories.extend(
        ["/bin", "/usr/bin", "/usr/local/bin", "/opt/homebrew/bin"]
            .into_iter()
            .map(PathBuf::from),
    );

    #[cfg(target_os = "windows")]
    if let Some(program_data) = env::var_os("ProgramData") {
        directories.push(PathBuf::from(program_data).join("chocolatey").join("bin"));
    }

    let home = if cfg!(target_os = "windows") {
        env::var_os("USERPROFILE")
    } else {
        env::var_os("HOME")
    };
    if let Some(home) = home {
        let home = PathBuf::from(home);
        #[cfg(target_os = "windows")]
        directories.push(home.join("scoop").join("shims"));
        #[cfg(not(target_os = "windows"))]
        {
            directories.push(home.join(".local").join("bin"));
            directories.push(home.join("bin"));
        }
    }

    directories
}

fn normalized_key(path: &Path) -> String {
    let value = path.to_string_lossy().into_owned();
    if cfg!(target_os = "windows") {
        value.to_lowercase()
    } else {
        value
    }
}

fn is_executable(path: &Path, metadata: &fs::Metadata) -> bool {
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = path;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(windows)]
    {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        true
    }
}

fn name_matches(filename: &str, names: &[String], prefixes: bool) -> bool {
    names.iter().any(|name| {
        let (filename, name) = if cfg!(target_os = "windows") {
            (filename.to_lowercase(), name.to_lowercase())
        } else {
            (filename.to_string(), name.to_string())
        };
        let filename_stem = if cfg!(target_os = "windows") {
            filename.strip_suffix(".exe").unwrap_or(&filename)
        } else {
            &filename
        };
        let name_stem = if cfg!(target_os = "windows") {
            name.strip_suffix(".exe").unwrap_or(&name)
        } else {
            &name
        };
        filename_stem == name_stem || (prefixes && filename_stem.starts_with(name_stem))
    })
}

fn add_candidate(
    path: PathBuf,
    seen: &mut HashSet<String>,
    candidates: &mut Vec<ExecutableCandidate>,
) {
    let Ok(metadata) = fs::metadata(&path) else {
        return;
    };
    if !is_executable(&path, &metadata) {
        return;
    }
    let Ok(canonical) = fs::canonicalize(&path) else {
        return;
    };
    if !seen.insert(normalized_key(&canonical)) {
        return;
    }
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return;
    };
    candidates.push(ExecutableCandidate {
        path: path.to_string_lossy().into_owned(),
        canonical_path: canonical.to_string_lossy().into_owned(),
        name: name.to_string(),
    });
}

fn find_executables_in_directories(
    options: &ExecutableSearchOptions,
    directories: impl IntoIterator<Item = PathBuf>,
) -> Vec<ExecutableCandidate> {
    let mut seen_directories = HashSet::new();
    let mut seen_candidates = HashSet::new();
    let mut candidates = Vec::new();

    for directory in directories {
        if !seen_directories.insert(normalized_key(&directory)) {
            continue;
        }

        if options.match_name_prefixes {
            let Ok(entries) = fs::read_dir(&directory) else {
                continue;
            };
            let mut paths = entries
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| {
                    path.file_name()
                        .and_then(|value| value.to_str())
                        .is_some_and(|name| name_matches(name, &options.names, true))
                })
                .collect::<Vec<_>>();
            paths.sort();
            for path in paths {
                add_candidate(path, &mut seen_candidates, &mut candidates);
            }
        } else {
            for name in &options.names {
                add_candidate(directory.join(name), &mut seen_candidates, &mut candidates);
                #[cfg(target_os = "windows")]
                if Path::new(name).extension().is_none() {
                    add_candidate(
                        directory.join(format!("{name}.exe")),
                        &mut seen_candidates,
                        &mut candidates,
                    );
                }
            }
        }
    }
    candidates
}

pub fn find_executables(options: &ExecutableSearchOptions) -> Result<Vec<ExecutableCandidate>> {
    validate_options(options)?;
    let mut directories = options
        .additional_paths
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    directories.extend(standard_search_directories());
    Ok(find_executables_in_directories(options, directories))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn temporary_directory() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "kokorobox-native-executables-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create test directory");
        path
    }

    fn write_executable(path: &Path) {
        fs::write(path, b"test").expect("write test executable");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o755))
                .expect("mark test file executable");
        }
    }

    #[test]
    fn validates_names_and_additional_paths() {
        for name in ["", "../mihomo", "folder/mihomo", "folder\\mihomo"] {
            let options = ExecutableSearchOptions {
                names: vec![name.to_string()],
                ..Default::default()
            };
            assert!(validate_options(&options).is_err(), "accepted {name:?}");
        }
        let options = ExecutableSearchOptions {
            names: vec!["mihomo".to_string()],
            additional_paths: vec!["relative".to_string()],
            ..Default::default()
        };
        assert!(validate_options(&options).is_err());
    }

    #[test]
    fn finds_executables_and_deduplicates_canonical_paths() {
        let directory = temporary_directory();
        let executable = directory.join("mihomo");
        write_executable(&executable);
        let options = ExecutableSearchOptions {
            names: vec!["mihomo".to_string()],
            ..Default::default()
        };
        let candidates =
            find_executables_in_directories(&options, [directory.clone(), directory.clone()]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, "mihomo");
        fs::remove_dir_all(directory).expect("remove test directory");
    }

    #[test]
    fn supports_explicit_prefix_matching() {
        let directory = temporary_directory();
        write_executable(&directory.join("mihomo-alpha"));
        write_executable(&directory.join("unrelated"));
        let options = ExecutableSearchOptions {
            names: vec!["mihomo".to_string()],
            match_name_prefixes: true,
            ..Default::default()
        };
        let candidates = find_executables_in_directories(&options, [directory.clone()]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, "mihomo-alpha");
        fs::remove_dir_all(directory).expect("remove test directory");
    }
}
