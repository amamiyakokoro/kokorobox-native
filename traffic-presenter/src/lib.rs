#![deny(clippy::all)]

pub mod runtime;

use serde::{Deserialize, Serialize};
use std::fmt;

pub const PROTOCOL_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenterLayout {
    Horizontal,
    Stacked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenterTheme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrafficSnapshot {
    pub up: u64,
    pub down: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresenterState {
    pub visible: bool,
    pub layout: PresenterLayout,
    pub theme: PresenterTheme,
    pub traffic: Option<TrafficSnapshot>,
}

impl Default for PresenterState {
    fn default() -> Self {
        Self {
            visible: false,
            layout: PresenterLayout::Stacked,
            theme: PresenterTheme::System,
            traffic: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenterTransition {
    Updated,
    Shutdown,
}

impl PresenterState {
    pub fn apply(&mut self, command: PresenterCommand) -> PresenterTransition {
        match command {
            PresenterCommand::Configure {
                visible,
                layout,
                theme,
                ..
            } => {
                self.visible = visible;
                self.layout = layout;
                self.theme = theme;
                PresenterTransition::Updated
            }
            PresenterCommand::Traffic { up, down, .. } => {
                self.traffic = Some(TrafficSnapshot { up, down });
                PresenterTransition::Updated
            }
            PresenterCommand::Shutdown { .. } => PresenterTransition::Shutdown,
        }
    }

    pub fn upload_label(&self) -> String {
        self.traffic
            .map(|traffic| format!("↑ {}/s", format_rate(traffic.up)))
            .unwrap_or_else(|| "↑ —".to_owned())
    }

    pub fn download_label(&self) -> String {
        self.traffic
            .map(|traffic| format!("↓ {}/s", format_rate(traffic.down)))
            .unwrap_or_else(|| "↓ —".to_owned())
    }

    pub fn combined_label(&self) -> String {
        let separator = match self.layout {
            PresenterLayout::Horizontal => "  ",
            PresenterLayout::Stacked => "\n",
        };
        format!(
            "{}{}{}",
            self.upload_label(),
            separator,
            self.download_label()
        )
    }

    pub fn single_line_label(&self) -> String {
        format!("{}  {}", self.upload_label(), self.download_label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PresenterCommand {
    Configure {
        version: u8,
        visible: bool,
        layout: PresenterLayout,
        theme: PresenterTheme,
    },
    Traffic {
        version: u8,
        up: u64,
        down: u64,
    },
    Shutdown {
        version: u8,
    },
}

impl PresenterCommand {
    pub fn version(&self) -> u8 {
        match self {
            Self::Configure { version, .. }
            | Self::Traffic { version, .. }
            | Self::Shutdown { version } => *version,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    InvalidJson(String),
    UnsupportedVersion(u8),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(message) => write!(formatter, "invalid presenter command: {message}"),
            Self::UnsupportedVersion(version) => {
                write!(
                    formatter,
                    "unsupported presenter protocol version: {version}"
                )
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

pub fn parse_command(line: &str) -> Result<PresenterCommand, ProtocolError> {
    let command: PresenterCommand = serde_json::from_str(line)
        .map_err(|error| ProtocolError::InvalidJson(error.to_string()))?;
    if command.version() != PROTOCOL_VERSION {
        return Err(ProtocolError::UnsupportedVersion(command.version()));
    }
    Ok(command)
}

pub fn format_rate(bytes_per_second: u64) -> String {
    const UNITS: [&str; 9] = ["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];

    if bytes_per_second < 1024 {
        return format!("{bytes_per_second} B");
    }

    let mut value = bytes_per_second as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    let fixed = format!("{value:.2}");
    let number = match fixed.len() {
        0..=5 => fixed,
        6 => format!("{value:.1}"),
        _ => format!("{value:.0}"),
    };
    format!("{number} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versioned_traffic_command() {
        let command = parse_command(r#"{"version":1,"type":"traffic","up":1024,"down":2048}"#)
            .expect("traffic command");
        assert_eq!(
            command,
            PresenterCommand::Traffic {
                version: PROTOCOL_VERSION,
                up: 1024,
                down: 2048,
            }
        );
    }

    #[test]
    fn rejects_unknown_fields_and_protocol_versions() {
        assert!(matches!(
            parse_command(r#"{"version":2,"type":"shutdown"}"#),
            Err(ProtocolError::UnsupportedVersion(2))
        ));
        assert!(matches!(
            parse_command(r#"{"version":1,"type":"shutdown","extra":true}"#),
            Err(ProtocolError::InvalidJson(_))
        ));
    }

    #[test]
    fn formats_rates_like_the_desktop_client() {
        assert_eq!(format_rate(0), "0 B");
        assert_eq!(format_rate(1023), "1023 B");
        assert_eq!(format_rate(1024), "1.00 KB");
        assert_eq!(format_rate(12 * 1024), "12.00 KB");
        assert_eq!(format_rate(100 * 1024), "100.0 KB");
        assert_eq!(format_rate(1023 * 1024), "1023 KB");
        assert_eq!(format_rate(1024 * 1024), "1.00 MB");
    }

    #[test]
    fn applies_commands_to_shared_presenter_state() {
        let mut state = PresenterState::default();
        assert_eq!(state.upload_label(), "↑ —");

        assert_eq!(
            state.apply(PresenterCommand::Configure {
                version: PROTOCOL_VERSION,
                visible: true,
                layout: PresenterLayout::Horizontal,
                theme: PresenterTheme::Dark,
            }),
            PresenterTransition::Updated
        );
        assert_eq!(
            state.apply(PresenterCommand::Traffic {
                version: PROTOCOL_VERSION,
                up: 1024,
                down: 2048,
            }),
            PresenterTransition::Updated
        );

        assert!(state.visible);
        assert_eq!(state.combined_label(), "↑ 1.00 KB/s  ↓ 2.00 KB/s");
        assert_eq!(state.single_line_label(), "↑ 1.00 KB/s  ↓ 2.00 KB/s");
        assert_eq!(
            state.apply(PresenterCommand::Shutdown {
                version: PROTOCOL_VERSION,
            }),
            PresenterTransition::Shutdown
        );
    }
}
