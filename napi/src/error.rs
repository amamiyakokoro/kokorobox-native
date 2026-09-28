pub(crate) fn map_err(error: impl std::fmt::Display) -> napi::Error {
    // Alternate display preserves anyhow's complete context chain.
    napi::Error::from_reason(format!("{error:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_retain_the_underlying_cause() {
        let error = kokorobox_native::file_to_data_url("C:/kokorobox-log-test/missing-file")
            .unwrap_err()
            .context("Read Native file failed");
        let error = map_err(error);
        assert!(error.reason.starts_with("Read Native file failed: "));
        assert!(error.reason.len() > "Read Native file failed: ".len());
    }
}
