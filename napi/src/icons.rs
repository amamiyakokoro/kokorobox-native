use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::error::map_err;

#[napi]
pub fn file_to_data_url(path: String) -> Result<String> {
    kokorobox_native::file_to_data_url(path).map_err(map_err)
}

#[napi]
pub fn get_app_name(path: String) -> Result<String> {
    kokorobox_native::get_app_name(path).map_err(map_err)
}
