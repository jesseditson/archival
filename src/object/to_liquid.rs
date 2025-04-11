use crate::{fields::ObjectValues, FieldValue, ObjectDefinition};
use liquid::model::{KString, ObjectIndex};
use liquid_core::{Value, ValueView};

pub fn object_to_liquid(
    object_values: &ObjectValues,
    definition: &ObjectDefinition,
) -> liquid::model::Object {
    let mut values: Vec<(KString, Value)> = definition
        .fields
        .keys()
        .map(|k| {
            (
                KString::from_ref(k.as_index()),
                object_values
                    .get(k)
                    .map(|v| v.to_value())
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
                    .map(|v| v.typed_objects(child_def))
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
