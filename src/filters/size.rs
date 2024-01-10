use liquid_core::Result;
use liquid_core::Runtime;
use liquid_core::Value;
use liquid_core::ValueView;
use liquid_core::{Display_filter, Filter, FilterReflection, ParseFilter};

#[derive(Debug, Default, Display_filter)]
#[name = "size"]
pub struct SizeFilter;

impl Filter for SizeFilter {
    fn evaluate(&self, input: &dyn ValueView, _runtime: &dyn Runtime) -> Result<Value> {
        if let Some(x) = input.as_scalar() {
            Ok(Value::scalar(x.to_kstr().len() as i64))
        } else if let Some(x) = input.as_array() {
            Ok(Value::scalar(x.size()))
        } else if let Some(x) = input.as_object() {
            Ok(Value::scalar(x.size()))
        } else {
            Ok(Value::scalar(0i64))
        }
    }
}

#[derive(Clone, ParseFilter, FilterReflection)]
#[filter(name = "size", description = "tests helper", parsed(SizeFilter))]
pub struct SizeFilterParser;
