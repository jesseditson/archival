use crate::{
    field_value::FieldValue,
    reserved_fields::{self, is_reserved_field, reserved_field_from_str, ReservedFieldError},
};
use liquid_core::model::DateTime;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, error::Error, fmt::Debug};
use thiserror::Error;
use toml::Table;

#[derive(Error, Debug, Clone)]
pub enum InvalidFieldError {
    #[error("unrecognized type {0}")]
    UnrecognizedType(String),
    #[error("type mismatch for field {field:?} of type {field_type:?} ({value:?})")]
    TypeMismatch {
        field: String,
        field_type: String,
        value: String,
    },
    #[error("invalid child {key:?}[{index:?}] {child:?}")]
    InvalidChild {
        key: String,
        index: usize,
        child: String,
    },
    #[error("not an array: {key:?} ({value:?})")]
    NotAnArray { key: String, value: String },
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub enum FieldType {
    String,
    Number,
    Date,
    Markdown,
    Boolean,
}

impl FieldType {
    fn from_str(string: &str) -> Result<FieldType, InvalidFieldError> {
        match string {
            "string" => Ok(FieldType::String),
            "number" => Ok(FieldType::Number),
            "date" => Ok(FieldType::Date),
            "markdown" => Ok(FieldType::Markdown),
            "boolean" => Ok(FieldType::Boolean),
            _ => Err(InvalidFieldError::UnrecognizedType(string.to_string())),
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            Self::String => "string",
            Self::Number => "number",
            Self::Date => "date",
            Self::Markdown => "markdown",
            Self::Boolean => "boolean",
        }
    }
    pub fn to_string(&self) -> String {
        self.to_str().to_string()
    }

    pub fn default_value(&self) -> FieldValue {
        match self {
            Self::String => FieldValue::String("".to_string()),
            Self::Number => FieldValue::Number(0.0),
            Self::Date => FieldValue::Date(DateTime::now()),
            Self::Markdown => FieldValue::Markdown("".to_string()),
            Self::Boolean => FieldValue::Boolean(false),
        }
    }
}

pub type ObjectDefinitions = HashMap<String, ObjectDefinition>;

#[derive(Debug, Deserialize, Serialize, Clone)]
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
    fn from_definition(name: &str, definition: &Table) -> Result<ObjectDefinition, Box<dyn Error>> {
        let mut object = ObjectDefinition::new(name);
        for (key, m_value) in definition {
            if let Some(child_table) = m_value.as_table() {
                object.children.insert(
                    key.clone(),
                    ObjectDefinition::from_definition(key, child_table)?,
                );
            } else if let Some(value) = m_value.as_str() {
                if key == reserved_fields::TEMPLATE {
                    object.template = Some(value.to_string());
                } else if is_reserved_field(key) {
                    return Err(Box::new(ReservedFieldError {
                        field: reserved_field_from_str(key),
                    }));
                } else {
                    object
                        .fields
                        .insert(key.clone(), FieldType::from_str(value)?);
                }
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
pub mod tests {

    use super::*;

    pub fn artist_and_page_definition_str() -> &'static str {
        "[artist]
        name = \"string\"
        template = \"artist\"
        [artist.tour_dates]
        date = \"date\"
        ticket_link = \"string\"
        [artist.numbers]
        number = \"number\"
        
        [page]
        content = \"markdown\"
        [page.links]
        url = \"string\""
    }

    #[test]
    fn parsing() -> Result<(), Box<dyn Error>> {
        let table: Table = toml::from_str(artist_and_page_definition_str())?;
        let defs = ObjectDefinition::from_table(&table)?;

        println!("{:?}", defs);

        assert_eq!(defs.keys().len(), 2);
        assert!(defs.get("artist").is_some());
        assert!(defs.get("page").is_some());
        let artist = defs.get("artist").unwrap();
        assert!(artist.fields.get("name").is_some());
        assert_eq!(artist.fields.get("name").unwrap(), &FieldType::String);
        assert!(
            artist.fields.get("template").is_none(),
            "did not copy the template reserved field"
        );
        assert!(artist.template.is_some());
        assert_eq!(artist.template, Some("artist".to_string()));
        assert_eq!(artist.children.len(), 2);
        assert!(artist.children.get("tour_dates").is_some());
        assert!(artist.children.get("numbers").is_some());
        let tour_dates = artist.children.get("tour_dates").unwrap();
        assert!(tour_dates.fields.get("date").is_some());
        assert_eq!(tour_dates.fields.get("date").unwrap(), &FieldType::Date);
        assert!(tour_dates.fields.get("ticket_link").is_some());
        assert_eq!(
            tour_dates.fields.get("ticket_link").unwrap(),
            &FieldType::String
        );
        let numbers = artist.children.get("numbers").unwrap();
        assert!(numbers.fields.get("number").is_some());
        assert_eq!(numbers.fields.get("number").unwrap(), &FieldType::Number);

        let page = defs.get("page").unwrap();
        assert!(page.fields.get("content").is_some());
        assert_eq!(page.fields.get("content").unwrap(), &FieldType::Markdown);
        assert_eq!(page.children.len(), 1);
        assert!(page.children.get("links").is_some());
        let links = page.children.get("links").unwrap();
        assert!(links.fields.get("url").is_some());
        assert_eq!(links.fields.get("url").unwrap(), &FieldType::String);

        Ok(())
    }
}
