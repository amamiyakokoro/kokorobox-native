use std::collections::HashMap;

use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

#[napi(object)]
#[derive(Default)]
pub struct RuleConvertOptions {
    #[napi(ts_type = "'mihomo' | 'general' | 'egern' | 'sing-box'")]
    pub input_target: Option<String>,
    #[napi(
        ts_type = "'yaml' | 'mrs' | 'text' | 'json' | 'srs' | 'domainset' | 'ruleset' | 'ipset'"
    )]
    pub input_format: Option<String>,
    #[napi(ts_type = "'auto' | 'domain' | 'ip' | 'classical'")]
    pub input_behavior: Option<String>,
    #[napi(ts_type = "'mihomo' | 'general' | 'egern' | 'sing-box'")]
    pub output_target: Option<String>,
    #[napi(
        ts_type = "'mrs' | 'text' | 'yaml' | 'json' | 'srs' | 'domainset' | 'ruleset' | 'ipset'"
    )]
    pub output_format: Option<String>,
    #[napi(ts_type = "'auto' | 'domain' | 'ip' | 'classical'")]
    pub output_behavior: Option<String>,
}

#[napi(object)]
pub struct RuleOutputInfo {
    pub behavior: Option<String>,
    pub format: String,
    pub count: u32,
}

#[napi(object)]
pub struct RuleSkippedItem {
    pub rule: String,
    pub reason: String,
}

#[napi(object)]
pub struct RuleStringResult {
    #[napi(ts_type = "'rules'")]
    pub kind: String,
    pub outputs: HashMap<String, String>,
    pub info: HashMap<String, RuleOutputInfo>,
    pub skipped: Vec<RuleSkippedItem>,
}

#[napi]
pub fn file_to_str(path: String, options: Option<RuleConvertOptions>) -> Result<RuleStringResult> {
    kokorobox_native::rule_file_to_string(path, to_core_options(options))
        .map(to_js_result)
        .map_err(map_err)
}

fn to_core_options(options: Option<RuleConvertOptions>) -> kokorobox_native::RuleConvertOptions {
    let options = options.unwrap_or_default();
    kokorobox_native::RuleConvertOptions {
        input_target: options.input_target,
        input_format: options.input_format,
        input_behavior: options.input_behavior,
        output_target: options.output_target,
        output_format: options.output_format,
        output_behavior: options.output_behavior,
    }
}

fn to_js_result(result: kokorobox_native::RuleStringResult) -> RuleStringResult {
    RuleStringResult {
        kind: result.kind,
        outputs: result.outputs,
        info: result
            .info
            .into_iter()
            .map(|(name, info)| {
                (
                    name,
                    RuleOutputInfo {
                        behavior: info.behavior,
                        format: info.format,
                        count: info.count,
                    },
                )
            })
            .collect(),
        skipped: result
            .skipped
            .into_iter()
            .map(|item| RuleSkippedItem {
                rule: item.rule,
                reason: item.reason,
            })
            .collect(),
    }
}
