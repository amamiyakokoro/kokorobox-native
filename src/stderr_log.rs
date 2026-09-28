use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

// Sidecar stderr uses JSON Lines; stdout remains reserved for its protocol.
pub fn write(level: &str, target: &str, message: std::fmt::Arguments<'_>) {
    write_to(&mut io::stderr().lock(), level, target, message);
}

pub fn write_to(
    writer: &mut impl Write,
    level: &str,
    target: &str,
    message: std::fmt::Arguments<'_>,
) {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let entry = serde_json::json!({
        "timestamp": timestamp, "level": level, "target": target,
        "msg": message.to_string().chars().take(4096).collect::<String>(),
    });
    let _ = writeln!(writer, "{entry}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_records_preserve_levels_and_escape_newlines() {
        let mut output = Vec::new();
        write_to(
            &mut output,
            "warn",
            "presenter",
            format_args!("quoted \"message\"\nnext line"),
        );
        let entry: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(entry["level"], "warn");
        assert_eq!(entry["target"], "presenter");
        assert_eq!(entry["msg"], "quoted \"message\"\nnext line");
        assert!(entry["timestamp"].as_u64().is_some());
        assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
    }

    #[test]
    fn sidecar_messages_are_bounded_without_splitting_unicode() {
        let mut output = Vec::new();
        write_to(
            &mut output,
            "error",
            "presenter",
            format_args!("{}", "😀".repeat(5000)),
        );
        let entry: serde_json::Value = serde_json::from_slice(&output).unwrap();
        let message = entry["msg"].as_str().unwrap();
        assert_eq!(message.chars().count(), 4096);
        assert!(!message.contains('\u{fffd}'));
    }
}
