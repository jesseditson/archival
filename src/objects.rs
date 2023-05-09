use std::{collections::HashMap, error::Error, fmt};

use serde::{Deserialize, Serialize};
use toml::Table;

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

#[derive(Deserialize, Serialize)]
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
            "Date" => Ok(FieldType::Date),
            "Markdown" => Ok(FieldType::Markdown),
            _ => Err(InvalidFieldError {
                field: string.to_string(),
            }),
        }
    }
}

pub type Objects = HashMap<String, ObjectDefinition>;

#[derive(Deserialize, Serialize)]
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
                    // Reserved names
                    "template" => {
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
