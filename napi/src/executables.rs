use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

#[napi(object)]
pub struct JsExecutableSearchOptions {
    pub names: Vec<String>,
    pub additional_paths: Option<Vec<String>>,
    pub match_name_prefixes: Option<bool>,
}

#[napi(object)]
pub struct JsExecutableCandidate {
    pub path: String,
    pub canonical_path: String,
    pub name: String,
}

impl From<kokorobox_native::ExecutableCandidate> for JsExecutableCandidate {
    fn from(value: kokorobox_native::ExecutableCandidate) -> Self {
        Self {
            path: value.path,
            canonical_path: value.canonical_path,
            name: value.name,
        }
    }
}

pub struct FindExecutablesTask {
    options: kokorobox_native::ExecutableSearchOptions,
}

#[napi]
impl Task for FindExecutablesTask {
    type Output = Vec<kokorobox_native::ExecutableCandidate>;
    type JsValue = Vec<JsExecutableCandidate>;

    fn compute(&mut self) -> Result<Self::Output> {
        kokorobox_native::find_executables(&self.options).map_err(map_err)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into_iter().map(Into::into).collect())
    }
}

#[napi]
pub fn find_executables(options: JsExecutableSearchOptions) -> AsyncTask<FindExecutablesTask> {
    AsyncTask::new(FindExecutablesTask {
        options: kokorobox_native::ExecutableSearchOptions {
            names: options.names,
            additional_paths: options.additional_paths.unwrap_or_default(),
            match_name_prefixes: options.match_name_prefixes.unwrap_or(false),
        },
    })
}
