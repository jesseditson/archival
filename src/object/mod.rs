pub use crate::value_path::ValuePath;
use crate::{
    fields::{FieldValue, InvalidFieldError, ObjectValues},
    object_definition::ObjectDefinition,
    reserved_fields::{self, is_reserved_field},
};
use liquid::{
    model::{KString, ObjectIndex, Value},
    ObjectView, ValueView,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt::Debug, path::Path};
use toml::Table;
use tracing::{instrument, warn};
mod object_entry;
pub use object_entry::ObjectEntry;

#[derive(Debug, ObjectView, ValueView, Deserialize, Serialize, Clone)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct Object {
    pub filename: String,
    pub object_name: String,
    pub order: i32,
    pub path: String,
    pub values: ObjectValues,
}

impl Object {
    #[instrument(skip(definition, table))]
    pub fn values_from_table(
        file: &Path,
        table: &Table,
        definition: &ObjectDefinition,
    ) -> Result<ObjectValues, Box<dyn Error>> {
        // liquid-rust only supports strict parsing. This is reasonable but we
        // also want to allow empty root keys, so we fill in defaults for any
        // missing definition keys
        let mut values = definition.empty_object();
        for (key, value) in table {
            if let Some(field_type) = definition.fields.get(&key.to_string()) {
                // Primitive values
                let field_value = FieldValue::from_toml(key, field_type, value)?;
                values.insert(key.to_string(), field_value);
            } else if let Some(child_def) = definition.children.get(&key.to_string()) {
                // Children
                let m_objects = value
                    .as_array()
                    .ok_or_else(|| InvalidFieldError::NotAnArray {
                        key: key.to_string(),
                        value: value.to_string(),
                    })?;
                let mut objects: Vec<ObjectValues> = Vec::new();
                for (index, object) in m_objects.iter().enumerate() {
                    let table =
                        object
                            .as_table()
                            .ok_or_else(|| InvalidFieldError::InvalidChild {
                                key: key.to_owned(),
                                index,
                                child: value.to_string(),
                            })?;
                    let object = Object::values_from_table(file, table, child_def)?;
                    objects.push(object);
                }
                let field_value = FieldValue::Objects(objects);
                values.insert(key.to_string(), field_value);
            } else if !is_reserved_field(key) {
                warn!("{}: unknown field {}", file.display(), key);
            }
        }
        Ok(values)
    }

    #[instrument(skip(definition, table))]
    pub fn from_table(
        definition: &ObjectDefinition,
        file: &Path,
        table: &Table,
    ) -> Result<Object, Box<dyn Error>> {
        let values = Object::values_from_table(file, table, definition)?;
        let mut order = -1;
        if let Some(t_order) = table.get(reserved_fields::ORDER) {
            if let Some(int_order) = t_order.as_integer() {
                order = int_order as i32;
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
        order: i32,
    ) -> Result<Self, Box<dyn Error>> {
        let path = Path::new(&definition.name).join(filename);
        let empty = Table::new();
        let values = Object::values_from_table(&path, &empty, definition)?;
        Ok(Self {
            filename: filename.to_owned(),
            object_name: definition.name.clone(),
            path: path.to_string_lossy().to_string(),
            order,
            values,
        })
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        let mut write_obj = Table::new();
        write_obj.insert("order".to_string(), toml::Value::Integer(self.order as i64));
        for (key, val) in &self.values {
            write_obj.insert(key.to_string(), val.into());
        }
        toml::to_string_pretty(&write_obj)
    }

    pub fn liquid_object(&self) -> Value {
        let mut values: liquid::model::Object = self
            .values
            .iter()
            .map(|(k, v)| (KString::from_ref(k.as_index()), v.to_value()))
            .collect();
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
        let defs =
            ObjectDefinition::from_table(&toml::from_str(artist_and_example_definition_str())?)?;
        let table: Table = toml::from_str(artist_object_str())?;
        let obj = Object::from_table(
            defs.get("artist").unwrap(),
            Path::new("tormenta-rey"),
            &table,
        )?;
        assert_eq!(obj.order, 1);
        assert_eq!(obj.object_name, "artist");
        assert_eq!(obj.filename, "tormenta-rey");
        assert_eq!(obj.values.len(), 4);
        assert!(obj.values.get("name").is_some());
        assert!(obj.values.get("tour_dates").is_some());
        assert!(obj.values.get("numbers").is_some());
        assert!(obj.values.get("videos").is_some());
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
            if let FieldValue::File(vf) = vf {
                assert_eq!(vf.sha, "fake-sha");
                assert_eq!(vf.name, Some("Video Name".to_string()));
                assert_eq!(vf.filename, "video.mp4");
                assert_eq!(vf.mime, "video/mpeg4");
                assert_eq!(vf.url, format!("{}/fake-sha", FieldConfig::get().cdn_url));
            }
        }

        Ok(())
    }
}
