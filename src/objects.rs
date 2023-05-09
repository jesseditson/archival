use std::{collections::HashMap, error::Error, fmt};

use super::reserved_fields;
use liquid::model::DateTime;
use serde::{Deserialize, Serialize};
use toml::{Table, Value};

// Instances

pub type ObjectValues = HashMap<String, FieldValue>;

#[derive(Deserialize, Serialize)]
pub enum FieldValue {
    String(String),
    Number(f64),
    Date(DateTime),
    Objects(Vec<ObjectValues>),
}
impl FieldValue {
    pub fn from_toml(
        key: String,
        field_type: FieldType,
        value: Value,
    ) -> Result<FieldValue, Box<dyn Error>> {
        if let Some(m_objects) = value.as_array() {
            let m_objects = value.as_array().ok_or(InvalidFieldError {
                field: key.to_string(),
            })?;
            let mut objects: Vec<ObjectValues> = Vec::new();
            for object in m_objects {
                // TODO: object is a hash, make into an ObjectValues hashmap and
                // push to objects
            }
            return Ok(FieldValue::Objects(objects));
        }
        match field_type {
            FieldType::String => Ok(FieldValue::String(
                value
                    .as_str()
                    .ok_or(InvalidFieldError {
                        field: key.to_string(),
                    })?
                    .to_string(),
            )),
            FieldType::Number => Ok(FieldValue::Number(value.as_float().ok_or(
                InvalidFieldError {
                    field: key.to_string(),
                },
            )?)),
            FieldType::Date => {
                let date_str = value.as_str().ok_or(InvalidFieldError {
                    field: key.to_string(),
                })?;
                let liquid_date = DateTime::from_str(date_str).ok_or(InvalidFieldError {
                    field: key.to_string(),
                })?;
                // TODO: use this strategy for more accurate values
                // let toml_date = m_value.as_datetime().ok_or(InvalidFieldError {
                //     field: key.to_string(),
                // })?;
                // let date = toml_date.date.ok_or(InvalidFieldError {
                //     field: key.to_string(),
                // })?;
                // let offset = toml_date.offset.ok_or(InvalidFieldError {
                //     field: key.to_string(),
                // })?;
                // let liquid_date =
                //     DateTime::from_ymd(date.year as i32, date.month, date.day)
                //         .with_offset(offset);
                Ok(FieldValue::Date(liquid_date))
            }
            _ => Box::new(Err(InvalidFieldError {
                field: key.to_string(),
            })),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Object {
    pub name: String,
    pub object_name: String,
    pub order: i32,
    pub values: ObjectValues,
}

impl Object {
    pub fn from_table(
        definition: &ObjectDefinition,
        name: String,
        table: Table,
    ) -> Result<Object, Box<dyn Error>> {
        let mut object = Object {
            name,
            object_name: definition.name.clone(),
            order: -1,
            values: HashMap::new(),
        };
        for (key, value) in table {
            if let Some(field_type) = definition.fields.get(&key.to_string()) {
                object.values.insert(
                    key.to_string(),
                    FieldValue::from_toml(key, field_type, value)?,
                )
            }
        }
        Ok(object)
    }
}

// Definitions

#[derive(Debug, Clone)]
struct InvalidFieldError {
    field: String,
}
impl Error for InvalidFieldError {}
impl fmt::Display for InvalidFieldError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid field {}", self.field)
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub enum FieldType {
    String,
    Number,
    Date,
    Markdown,
}

impl FieldType {
    fn from_str(string: &str) -> Result<FieldType, InvalidFieldError> {
        match string {
            "string" => Ok(FieldType::String),
            "number" => Ok(FieldType::Number),
            "date" => Ok(FieldType::Date),
            "markdown" => Ok(FieldType::Markdown),
            _ => Err(InvalidFieldError {
                field: string.to_string(),
            }),
        }
    }
}

pub type Objects = HashMap<String, ObjectDefinition>;

#[derive(Deserialize, Serialize, Clone)]
pub struct ObjectDefinition {
    pub name: String,
    pub fields: HashMap<String, FieldType>,
    pub template: Option<String>,
    pub children: HashMap<String, ObjectDefinition>,
}

impl ObjectDefinition {
    pub fn new(name: &str) -> ObjectDefinition {
        ObjectDefinition {
            name: name.to_string(),
            fields: HashMap::new(),
            template: None,
            children: HashMap::new(),
        }
    }
    fn from_definition(
        name: &String,
        definition: &Table,
    ) -> Result<ObjectDefinition, Box<dyn Error>> {
        let mut object = ObjectDefinition::new(name);
        for (key, m_value) in definition {
            if let Some(value) = m_value.as_str() {
                match key.as_str() {
                    // Reserved fields
                    reserved_fields::TEMPLATE => {
                        object.template = Some(value.to_string());
                    }
                    _ => {
                        object
                            .fields
                            .insert(key.clone(), FieldType::from_str(value)?);
                    }
                }
            } else if let Some(child_table) = m_value.as_table() {
                object.children.insert(
                    key.clone(),
                    ObjectDefinition::from_definition(key, child_table)?,
                );
            }
        }
        Ok(object)
    }
    pub fn from_table(table: &Table) -> Result<HashMap<String, ObjectDefinition>, Box<dyn Error>> {
        let mut objects: HashMap<String, ObjectDefinition> = HashMap::new();
        for (name, m_def) in table.into_iter() {
            if let Some(def) = m_def.as_table() {
                objects.insert(name.clone(), ObjectDefinition::from_definition(name, def)?);
            }
        }
        Ok(objects)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
