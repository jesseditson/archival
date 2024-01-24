use std::{collections::HashMap, error::Error, fmt::Debug};

use crate::{
    field_value::{FieldValue, ObjectValues},
    object_definition::{InvalidFieldError, ObjectDefinition},
    reserved_fields::{self, is_reserved_field},
};

use liquid::{ObjectView, ValueView};
use serde::{Deserialize, Serialize};
use toml::Table;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm-fs", derive(typescript_type_def::TypeDef))]
pub enum ValuePathComponent {
    Key(String),
    Index(usize),
}

impl ValuePathComponent {
    pub fn key(name: &str) -> Self {
        Self::Key(name.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "wasm-fs", derive(typescript_type_def::TypeDef))]
pub struct ValuePath {
    path: Vec<ValuePathComponent>,
}

impl ValuePath {
    pub fn new() -> Self {
        Self { path: vec![] }
    }
    pub fn join(mut self, component: ValuePathComponent) -> Self {
        self.path.push(component);
        self
    }
    pub fn set_in_object(&self, object: &mut Object, value: FieldValue) {
        let mut i_path = self.path.iter().map(|v| match v {
            ValuePathComponent::Index(i) => ValuePathComponent::Index(*i),
            ValuePathComponent::Key(k) => ValuePathComponent::Key(k.to_owned()),
        });
        let mut last_val = None;
        while let Some(cmp) = i_path.next() {
            if last_val.is_none() {
                // At the root, we must have a key string
                if let ValuePathComponent::Key(k) = cmp {
                    if i_path.len() > 0 {
                        last_val = object.values.get_mut(&k);
                        continue;
                    } else {
                        object.values.insert(k, value);
                        break;
                    }
                }
            } else {
                // more than one level deep. We only allow accessing child
                // values, not children themselves - so this finds a child at
                // the index and then finds a key on it.
                if let Some(FieldValue::Objects(children)) = last_val {
                    if let ValuePathComponent::Index(index) = cmp {
                        if let Some(child) = children.get_mut(index) {
                            if let Some(ValuePathComponent::Key(k)) = i_path.next() {
                                if i_path.len() > 0 {
                                    last_val = child.get_mut(&k);
                                    continue;
                                } else {
                                    child.insert(k, value);
                                }
                            }
                        }
                    }
                }
            }
            break;
        }
    }
}

#[derive(Debug, ObjectView, ValueView, Deserialize, Serialize, Clone)]
pub struct Object {
    pub filename: String,
    pub object_name: String,
    pub order: i32,
    pub values: ObjectValues,
}

impl Object {
    pub fn values_from_table(
        table: &Table,
        definition: &ObjectDefinition,
    ) -> Result<ObjectValues, Box<dyn Error>> {
        let mut values: ObjectValues = HashMap::new();
        for (key, value) in table {
            if let Some(field_type) = definition.fields.get(&key.to_string()) {
                // Primitive values
                let field_value = FieldValue::from_toml(key, field_type, value)?;
                values.insert(key.to_string(), field_value);
            } else if let Some(child_def) = definition.children.get(&key.to_string()) {
                // Children
                let m_objects = value.as_array().ok_or(InvalidFieldError {
                    field: key.to_string(),
                    value: value.to_string(),
                })?;
                let mut objects: Vec<ObjectValues> = Vec::new();
                for (index, object) in m_objects.iter().enumerate() {
                    let table = object.as_table().ok_or(InvalidFieldError {
                        field: format!("{}: {}", key, index),
                        value: value.to_string(),
                    })?;
                    let object = Object::values_from_table(table, child_def)?;
                    objects.push(object);
                }
                let field_value = FieldValue::Objects(objects);
                values.insert(key.to_string(), field_value);
            } else if !is_reserved_field(key) {
                println!("Unknown field {}", key);
            }
        }
        // liquid-rust only supports strict parsing. This is reasonable but we
        // also want to allow empty root keys, so we fill in defaults for any
        // missing definition keys
        for (field, def) in &definition.fields {
            if !values.contains_key(field) {
                values.insert(field.to_owned(), def.default_value());
            }
        }
        for (cd, def) in &definition.children {
            if !values.contains_key(cd) {
                values.insert(def.name.to_owned(), FieldValue::Objects(vec![]));
            }
        }
        Ok(values)
    }

    pub fn from_table(
        definition: &ObjectDefinition,
        filename: &str,
        table: &Table,
    ) -> Result<Object, Box<dyn Error>> {
        let values = Object::values_from_table(table, definition)?;
        let mut order = -1;
        if let Some(t_order) = table.get(reserved_fields::ORDER) {
            if let Some(int_order) = t_order.as_integer() {
                order = int_order as i32;
            } else {
                println!("Invalid order {}", t_order);
            }
        }
        let object = Object {
            filename: filename.to_owned(),
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
        let empty = Table::new();
        let values = Object::values_from_table(&empty, definition)?;
        Ok(Self {
            filename: filename.to_owned(),
            object_name: definition.name.clone(),
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
        Ok(toml::to_string_pretty(&write_obj)?)
    }
}

#[cfg(test)]
mod tests {
    use liquid::model::DateTime;

    use crate::object_definition::tests::artist_and_page_definition_str;

    use super::*;

    fn artist_object_str() -> &'static str {
        "name = \"Tormenta Rey\"
        order = 1
      
      [[tour_dates]]
      date = \"12/22/2022\"
      ticket_link = \"foo.com\"
      
      [[numbers]]
      number = 2.57"
    }

    #[test]
    fn object_parsing() -> Result<(), Box<dyn Error>> {
        let defs =
            ObjectDefinition::from_table(&toml::from_str(artist_and_page_definition_str())?)?;
        let table: Table = toml::from_str(artist_object_str())?;
        let obj = Object::from_table(defs.get("artist").unwrap(), "tormenta-rey", &table)?;
        assert_eq!(obj.order, 1);
        assert_eq!(obj.object_name, "artist");
        assert_eq!(obj.filename, "tormenta-rey");
        assert_eq!(obj.values.len(), 3);
        assert!(obj.values.get("name").is_some());
        assert!(obj.values.get("tour_dates").is_some());
        assert!(obj.values.get("numbers").is_some());
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
                date.get("date").unwrap(),
                &FieldValue::Date(DateTime::from_ymd(2022, 12, 22))
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

        Ok(())
    }
}
