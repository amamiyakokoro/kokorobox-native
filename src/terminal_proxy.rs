use anyhow::Result;
#[cfg(not(target_os = "linux"))]
use anyhow::bail;

#[cfg(target_os = "linux")]
mod linux {
    use std::{
        env, fs,
        io::{ErrorKind, Write},
        os::unix::fs::OpenOptionsExt,
        path::{Path, PathBuf},
    };

    use anyhow::{Context, Result, anyhow, bail};
    use zbus::blocking::{Connection, Proxy};

    const MARKER: &str = "# Managed by KokoroBox.";
    const NAMES: [&str; 8] = [
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "no_proxy",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
    ];

    fn config_path() -> Result<PathBuf> {
        let home = env::var_os("HOME").ok_or_else(|| anyhow!("HOME is not set"))?;
        let config_home = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| PathBuf::from(home).join(".config"));
        Ok(config_home.join("environment.d/90-kokorobox-proxy.conf"))
    }

    fn validate_value(value: &str) -> Result<()> {
        if value.contains(['\0', '\r', '\n']) {
            bail!("Terminal proxy value contains an invalid line break or NUL");
        }
        Ok(())
    }

    fn quote(value: &str) -> Result<String> {
        validate_value(value)?;
        Ok(format!(
            "\"{}\"",
            value
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('`', "\\`")
                .replace('$', "$$")
        ))
    }

    fn assignments(host: &str, port: u16, bypass: &[String]) -> Result<Vec<String>> {
        if host.is_empty() || port == 0 {
            bail!("Terminal proxy host and port are required");
        }
        validate_value(host)?;
        for entry in bypass {
            validate_value(entry)?;
        }
        let proxy = format!("http://{host}:{port}");
        let no_proxy = bypass.join(",");
        Ok(NAMES
            .iter()
            .map(|name| {
                let value = if name.eq_ignore_ascii_case("no_proxy") {
                    &no_proxy
                } else {
                    &proxy
                };
                format!("{name}={value}")
            })
            .collect())
    }

    fn content(assignments: &[String]) -> Result<String> {
        let mut result = format!("{MARKER} Changes will be overwritten.\n");
        for assignment in assignments {
            let (name, value) = assignment
                .split_once('=')
                .ok_or_else(|| anyhow!("Invalid terminal proxy assignment"))?;
            result.push_str(name);
            result.push('=');
            result.push_str(&quote(value)?);
            result.push('\n');
        }
        Ok(result)
    }

    fn write_config(path: &Path, content: &str) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| anyhow!("Invalid terminal proxy path"))?;
        fs::create_dir_all(parent).context("Create terminal proxy directory")?;
        match fs::read_to_string(path) {
            Ok(existing) if !existing.starts_with(MARKER) => {
                bail!("Terminal proxy path is not owned by KokoroBox")
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("Read terminal proxy configuration"),
        }
        let mut random = [0u8; 8];
        getrandom::fill(&mut random).context("Create terminal proxy temporary name")?;
        let temporary = parent.join(format!(
            ".90-kokorobox-proxy-{}-{}.tmp",
            std::process::id(),
            u64::from_le_bytes(random)
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .context("Create terminal proxy temporary file")?;
        let result = (|| -> Result<()> {
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.context("Write terminal proxy configuration")
    }

    fn remove_managed_config(path: &Path) -> Result<bool> {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error).context("Read terminal proxy configuration"),
        };
        if !content.starts_with(MARKER) {
            return Ok(false);
        }
        match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error).context("Remove terminal proxy configuration"),
        }
    }

    fn update_user_manager(method: &str, values: &[String]) -> bool {
        let result = (|| -> Result<()> {
            let connection = Connection::session()?;
            let proxy = Proxy::new(
                &connection,
                "org.freedesktop.systemd1",
                "/org/freedesktop/systemd1",
                "org.freedesktop.systemd1.Manager",
            )?;
            proxy.call::<_, _, ()>(method, &(values,))?;
            Ok(())
        })();
        result.is_ok()
    }

    pub fn set(host: &str, port: u16, bypass: &[String]) -> Result<bool> {
        let assignments = assignments(host, port, bypass)?;
        write_config(&config_path()?, &content(&assignments)?)?;
        Ok(update_user_manager("SetEnvironment", &assignments))
    }

    pub fn clear() -> Result<Option<bool>> {
        if !remove_managed_config(&config_path()?)? {
            return Ok(None);
        }
        let names = NAMES.map(str::to_string);
        Ok(Some(update_user_manager("UnsetEnvironment", &names)))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn formats_fixed_environment_names_and_preserves_literal_values() {
            let values = assignments("127.0.0.1", 7890, &["a$b\\c\"d`e".into()]).unwrap();
            assert_eq!(values.len(), NAMES.len());
            assert_eq!(values[0], "http_proxy=http://127.0.0.1:7890");
            assert_eq!(values[3], "no_proxy=a$b\\c\"d`e");
            assert!(
                content(&values)
                    .unwrap()
                    .contains("no_proxy=\"a$$b\\\\c\\\"d\\`e\"")
            );
            assert!(assignments("bad\nname", 7890, &[]).is_err());
        }

        #[test]
        fn only_removes_its_own_configuration() {
            let path = env::temp_dir().join(format!(
                "kokorobox-terminal-proxy-test-{}",
                std::process::id()
            ));
            fs::write(&path, "USER=value\n").unwrap();
            assert!(!remove_managed_config(&path).unwrap());
            assert!(path.exists());
            assert!(write_config(&path, MARKER).is_err());
            fs::remove_file(&path).unwrap();
            write_config(
                &path,
                &content(&assignments("127.0.0.1", 7890, &[]).unwrap()).unwrap(),
            )
            .unwrap();
            assert!(remove_managed_config(&path).unwrap());
            assert!(!path.exists());
        }
    }
}

pub fn set_terminal_proxy_environment(host: &str, port: u16, bypass: &[String]) -> Result<bool> {
    #[cfg(target_os = "linux")]
    return linux::set(host, port, bypass);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (host, port, bypass);
        bail!("Terminal proxy environment is only available on Linux")
    }
}

pub fn clear_terminal_proxy_environment() -> Result<Option<bool>> {
    #[cfg(target_os = "linux")]
    return linux::clear();
    #[cfg(not(target_os = "linux"))]
    bail!("Terminal proxy environment is only available on Linux")
}
