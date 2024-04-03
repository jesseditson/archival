mod date_time;
pub(crate) mod field_type;
pub(crate) mod field_value;
mod file;
pub use date_time::DateTime;
pub use field_type::{FieldType, InvalidFieldError};
pub use field_value::{FieldValue, ObjectValues};
pub use file::File;
use once_cell::sync::Lazy;
use std::sync::{Mutex, MutexGuard};

use crate::constants::CDN_URL;

static CONFIG: Lazy<Mutex<FieldConfig>> = Lazy::new(|| Mutex::new(FieldConfig::default()));

pub struct FieldConfig {
    pub cdn_url: &'static str,
}

impl Default for FieldConfig {
    fn default() -> Self {
        Self { cdn_url: CDN_URL }
    }
}

impl FieldConfig {
    pub fn get<'a>() -> MutexGuard<'a, FieldConfig> {
        CONFIG.lock().expect("Invalid FieldConfig::get access")
    }
    pub fn set(fc: FieldConfig) {
        let mut c = CONFIG.lock().expect("Invalid FieldConfig::set call");
        *c = fc;
    }
}
