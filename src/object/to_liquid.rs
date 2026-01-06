use crate::{
    fields::{File, ObjectValues},
    FieldConfig, FieldValue, ObjectDefinition,
};
use liquid::model::{KString, ObjectIndex};
use liquid_core::{Value, ValueView};

pub fn object_to_liquid(
    object_values: &ObjectValues,
    definition: &ObjectDefinition,
    field_config: &FieldConfig,
) -> liquid::model::Object {
    let mut values: Vec<(KString, Value)> = definition
        .fields
        .keys()
        .map(|k| {
            (
                KString::from_ref(k.as_index()),
                object_values
                    .get(k)
                    .map(|v| v.to_liquid(field_config))
                    .unwrap_or_else(|| Value::Nil),
            )
        })
        .collect();
    let mut child_values: Vec<(KString, Value)> = definition
        .children
        .iter()
        .map(|(k, child_def)| {
            (
                KString::from_ref(k.as_index()),
                object_values
                    .get(k)
                    .map(|v| v.typed_objects(child_def, field_config))
                    .unwrap_or_else(|| Value::Array(vec![])),
            )
        })
        .collect();
    let mut meta_values: Vec<(KString, Value)> = object_values
        .iter()
        .filter_map(|(k, v)| {
            if let FieldValue::Meta(meta) = v {
                Some((KString::from_ref(k.as_index()), meta.to_liquid()))
            } else {
                None
            }
        })
        .collect();
    values.append(&mut meta_values);
    values.append(&mut child_values);
    values.into_iter().collect()
}

impl FieldValue {
    pub fn to_liquid(&self, field_config: &FieldConfig) -> liquid::model::Value {
        match self {
            FieldValue::File(file) => file.to_liquid(field_config),
            FieldValue::Oneof((t, v)) => match v.as_ref() {
                Some(v) => liquid::object!({
                    "type": t,
                    "value": v.to_liquid(field_config)
                })
                .into(),
                None => liquid::model::Value::Nil,
            },
            _ => self.to_value(),
        }
    }
}

impl File {
    pub fn to_liquid(&self, field_config: &FieldConfig) -> liquid::model::Value {
        let mut m = liquid::model::Object::new();
        for (k, v) in self.clone().into_map(Some(field_config)) {
            m.insert(k.into(), liquid::model::Value::scalar(v));
        }
        liquid_core::Value::Object(m)
    }
}
