pub use crate::value_path::{ValuePath, ValuePathComponent};
use crate::{
    events::AddObjectValue,
    fields::{FieldType, FieldValue, InvalidFieldError, ObjectValues},
    manifest::{EditorTypes, ManifestEditorTypeValidator},
    object_definition::ObjectDefinition,
    reserved_fields::{self, is_reserved_field},
};
use liquid::{
    model::{KString, Value},
    ObjectView, ValueView,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, error::Error, fmt::Debug, path::Path};
use to_liquid::object_to_liquid;
use toml::Table;
use tracing::{instrument, warn};
mod object_entry;
pub(crate) mod to_liquid;
pub use object_entry::ObjectEntry;

#[derive(Debug, ObjectView, ValueView, Deserialize, Serialize, Clone, PartialEq)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct Object {
    pub filename: String,
    pub object_name: String,
    pub order: Option<f64>,
    pub path: String,
    pub values: ObjectValues,
}

impl Object {
    #[instrument]
    pub fn validate(
        field_type: &FieldType,
        field_value: &FieldValue,
        custom_types: &EditorTypes,
    ) -> Result<(), InvalidFieldError> {
        // You can only define a validator via editor_types, which will always
        // create an alias type
        if let FieldType::Alias(a) = field_type {
            if let Some(custom_type) = custom_types.get(&a.1) {
                for validator in &custom_type.validate {
                    match validator {
                        ManifestEditorTypeValidator::Path(p) => {
                            if let Ok(validated_value) = p.path.get_value(field_value) {
                                if !p.validate.validate(&validated_value.to_string()) {
                                    return Err(InvalidFieldError::FailedValidation(
                                        validated_value.to_string(),
                                        p.validate.to_string(),
                                    ));
                                }
                            } else {
                                // Value not found - if our validator passes
                                // with an empty string, this is ok. Otherwise
                                // this is an error.
                                if !p.validate.validate("") {
                                    return Err(InvalidFieldError::FailedValidation(
                                        "(not found)".to_string(),
                                        p.validate.to_string(),
                                    ));
                                }
                            }
                        }
                        ManifestEditorTypeValidator::Value(v) => {
                            if !v.validate(&field_value.to_string()) {
                                return Err(InvalidFieldError::FailedValidation(
                                    field_value.to_string(),
                                    v.to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[instrument(skip(definition, table))]
    pub fn values_from_table(
        file: &Path,
        table: &Table,
        definition: &ObjectDefinition,
        custom_types: &EditorTypes,
        skip_validation: bool,
    ) -> Result<ObjectValues, Box<dyn Error>> {
        // liquid-rust only supports strict parsing. This is reasonable but we
        // also want to allow empty root keys, so we fill in defaults for any
        // missing definition keys
        let mut values = definition.empty_object();
        for (type_name, value) in table {
            let def_key = custom_types
                .get(type_name)
                .map(|i| &i.alias_of)
                .unwrap_or(type_name);
            if let Some(field_type) = definition.fields.get(def_key) {
                // Values
                let field_value = FieldValue::from_toml(def_key, field_type, value)?;
                if !skip_validation {
                    Object::validate(field_type, &field_value, custom_types)?;
                }
                values.insert(def_key.to_string(), field_value);
            } else if let Some(child_def) = definition.children.get(&def_key.to_string()) {
                // Children
                let m_objects = value
                    .as_array()
                    .ok_or_else(|| InvalidFieldError::NotAnArray {
                        key: def_key.to_string(),
                        value: value.to_string(),
                    })?;
                let mut objects: Vec<ObjectValues> = Vec::new();
                for (index, object) in m_objects.iter().enumerate() {
                    let table =
                        object
                            .as_table()
                            .ok_or_else(|| InvalidFieldError::InvalidChild {
                                key: def_key.to_owned(),
                                index,
                                child: value.to_string(),
                            })?;
                    let object = Object::values_from_table(
                        file,
                        table,
                        child_def,
                        custom_types,
                        skip_validation,
                    )?;
                    objects.push(object);
                }
                let field_value = FieldValue::Objects(objects);

                values.insert(def_key.to_string(), field_value);
            } else if !is_reserved_field(def_key) {
                warn!("{}: unknown field {}", file.display(), def_key);
            }
        }
        Ok(values)
    }

    #[instrument(skip(definition, table))]
    pub fn from_table(
        definition: &ObjectDefinition,
        file: &Path,
        table: &Table,
        custom_types: &EditorTypes,
        skip_validation: bool,
    ) -> Result<Object, Box<dyn Error>> {
        let values =
            Object::values_from_table(file, table, definition, custom_types, skip_validation)?;
        let mut order = None;
        if let Some(t_order) = table.get(reserved_fields::ORDER) {
            if let Some(int_order) = t_order.as_integer() {
                order = Some(int_order as f64);
            } else if let Some(float_order) = t_order.as_float() {
                order = Some(float_order);
            } else {
                warn!("Invalid order {}", t_order);
            }
        }
        let filename = file.file_name().unwrap().to_string_lossy().to_string();
        let object = Object {
            path: Path::new(&definition.name)
                .join(&filename)
                .to_string_lossy()
                .to_string(),
            filename,
            object_name: definition.name.clone(),
            order,
            values,
        };
        Ok(object)
    }

    pub fn from_def(
        definition: &ObjectDefinition,
        filename: &str,
        order: Option<f64>,
        defaults: Vec<AddObjectValue>,
    ) -> Result<Self, Box<dyn Error>> {
        let path = Path::new(&definition.name).join(filename);
        let values =
            Object::values_from_table(&path, &Table::new(), definition, &HashMap::new(), true)?;
        let mut object = Self {
            filename: filename.to_owned(),
            object_name: definition.name.clone(),
            path: path.to_string_lossy().to_string(),
            order,
            values,
        };
        for default in defaults {
            default
                .path
                .set_in_object(&mut object, Some(default.value))?;
        }
        Ok(object)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        let mut write_obj = Table::new();
        if let Some(order) = self.order {
            write_obj.insert(
                "order".to_string(),
                if order.fract() == 0. {
                    toml::Value::Integer(order as i64)
                } else {
                    toml::Value::Float(order)
                },
            );
        }
        for (key, val) in &self.values {
            if let Some(val) = val.into() {
                write_obj.insert(key.to_string(), val);
            }
        }
        toml::to_string_pretty(&write_obj)
    }

    pub fn liquid_object(&self, definition: &ObjectDefinition) -> Value {
        let mut values = object_to_liquid(&self.values, definition);
        // Reserved/special
        if values.contains_key("path") {
            panic!("Objects may not define path key.");
        }
        if values.contains_key("order") {
            panic!("Objects may not define order key.");
        }
        values.insert(KString::from_ref("path"), self.path.to_value());
        values.insert(KString::from_ref("order"), self.order.to_value());
        Value::Object(values)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::{
        fields::DateTime, object_definition::tests::artist_and_example_definition_str, FieldConfig,
    };

    use super::*;

    fn artist_object_str() -> &'static str {
        "name = \"Tormenta Rey\"
        order = 1
      
      [[tour_dates]]
      date = \"12/22/2022\"
      ticket_link = \"foo.com\"
    
      [[videos]]
      video = {sha = \"fake-sha\", name = \"Video Name\", filename = \"video.mp4\", mime = \"video/mpeg4\"}

      [[numbers]]
      number = 2.57"
    }

    #[test]
    fn object_parsing() -> Result<(), Box<dyn Error>> {
        let defs = ObjectDefinition::from_table(
            &toml::from_str(artist_and_example_definition_str())?,
            &HashMap::new(),
        )?;
        let table: Table = toml::from_str(artist_object_str())?;
        let obj = Object::from_table(
            defs.get("artist").unwrap(),
            Path::new("tormenta-rey"),
            &table,
            &HashMap::new(),
            false,
        )?;
        assert_eq!(obj.order, Some(1.));
        assert_eq!(obj.object_name, "artist");
        assert_eq!(obj.filename, "tormenta-rey");
        assert_eq!(obj.values.len(), 4);
        assert!(obj.values.contains_key("name"));
        assert!(obj.values.contains_key("tour_dates"));
        assert!(obj.values.contains_key("numbers"));
        assert!(obj.values.contains_key("videos"));
        assert_eq!(
            obj.values.get("name"),
            Some(&FieldValue::String("Tormenta Rey".to_string()))
        );
        let tour_dates = obj.values.get("tour_dates").unwrap();
        assert!(matches!(tour_dates, FieldValue::Objects { .. }));
        if let FieldValue::Objects(tour_dates) = tour_dates {
            assert_eq!(tour_dates.len(), 1);
            let date = tour_dates.first().unwrap();
            assert!(date.contains_key("date"));
            assert!(date.contains_key("ticket_link"));
            assert_eq!(
                date.get("date").unwrap().liquid_date(),
                FieldValue::Date(DateTime::from_ymd(2022, 12, 22)).liquid_date()
            );
            assert_eq!(
                date.get("ticket_link").unwrap(),
                &FieldValue::String("foo.com".to_string())
            );
        }
        let numbers = obj.values.get("numbers").unwrap();
        assert!(matches!(numbers, FieldValue::Objects { .. }));
        if let FieldValue::Objects(numbers) = numbers {
            assert_eq!(numbers.len(), 1);
            let num = numbers.first().unwrap();
            assert!(num.contains_key("number"));
            assert_eq!(num.get("number").unwrap(), &FieldValue::Number(2.57));
        }
        let videos = obj.values.get("videos").unwrap();
        assert!(matches!(videos, FieldValue::Objects { .. }));
        if let FieldValue::Objects(videos) = videos {
            assert_eq!(videos.len(), 1);
            let video = videos.first().unwrap();
            assert!(video.contains_key("video"));
            let vf = video.get("video").unwrap();
            assert!(matches!(vf, FieldValue::File(_)));
            println!("{:?}", vf);
            let fc = FieldConfig::get_global();
            if let FieldValue::File(vf) = vf {
                assert_eq!(vf.sha, "fake-sha");
                assert_eq!(vf.name, Some("Video Name".to_string()));
                assert_eq!(vf.filename, "video.mp4");
                assert_eq!(vf.mime, "video/mpeg4");
                assert_eq!(
                    vf.url,
                    format!("{}{}/fake-sha/video.mp4", fc.upload_prefix, fc.uploads_url)
                );
            }
        }

        Ok(())
    }
}
