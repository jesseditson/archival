use std::{
    collections::HashMap,
    error::Error,
    fmt::{self, Debug},
};

use crate::reserved_fields::{
    self, is_reserved_field, reserved_field_from_str, ReservedFieldError,
};

use liquid::{model, ObjectView, ValueView};
use serde::{Deserialize, Serialize};
use toml::{Table, Value};

// Instances

pub type ObjectValues = HashMap<String, FieldValue>;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub enum FieldValue {
    String(String),
    Number(f64),
    Date(model::DateTime),
    Objects(Vec<ObjectValues>),
}
// impl fmt::Debug for Position {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         f.debug_tuple("")
//          .field(&self.longitude)
//          .field(&self.latitude)
//          .finish()
//     }
// }

impl fmt::Display for FieldValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl ValueView for FieldValue {
    /// Get a `Debug` representation
    fn as_debug(&self) -> &dyn fmt::Debug {
        self
    }
    /// A `Display` for a `BoxedValue` rendered for the user.
    fn render(&self) -> model::DisplayCow<'_> {
        model::DisplayCow::Owned(Box::new(self))
    }
    /// A `Display` for a `Value` as source code.
    fn source(&self) -> model::DisplayCow<'_> {
        model::DisplayCow::Owned(Box::new(self))
    }

    /// Report the data type (generally for error reporting).
    fn type_name(&self) -> &'static str {
        match self {
            FieldValue::String(_) => "string",
            FieldValue::Number(_) => "number",
            FieldValue::Date(_) => "date",
            FieldValue::Objects(_) => "objects",
        }
    }
    /// Interpret as a string.
    fn to_kstr(&self) -> model::KStringCow<'_> {
        match self {
            _ => model::KStringCow::from(self.to_string()),
        }
    }
    /// Query the value's state
    fn query_state(&self, state: model::State) -> bool {
        match state {
            model::State::Truthy => false,
            model::State::DefaultValue => false,
            model::State::Empty => false,
            model::State::Blank => false,
        }
    }

    fn as_scalar(&self) -> Option<model::ScalarCow<'_>> {
        match self {
            FieldValue::String(s) => Some(model::ScalarCow::new(s)),
            FieldValue::Number(n) => Some(model::ScalarCow::new(n.clone())),
            // TODO: should be able to return a datetime value here
            FieldValue::Date(d) => Some(model::ScalarCow::new(d.clone())),
            _ => None,
        }
    }
    fn as_array(&self) -> Option<&dyn model::ArrayView> {
        match self {
            FieldValue::Objects(a) => Some(a),
            _ => None,
        }
    }

    fn to_value(&self) -> liquid::model::Value {
        match self {
            FieldValue::String(_) => self.as_scalar().to_value(),
            FieldValue::Number(_) => self.as_scalar().to_value(),
            FieldValue::Date(_) => self.as_scalar().to_value(),
            FieldValue::Objects(_) => self.as_array().to_value(),
        }
    }
}

impl FieldValue {
    pub fn from_toml(
        key: &String,
        field_type: &FieldType,
        value: &Value,
    ) -> Result<FieldValue, Box<dyn Error>> {
        match field_type {
            FieldType::String => Ok(FieldValue::String(
                value
                    .as_str()
                    .ok_or(InvalidFieldError {
                        field: key.to_string(),
                        value: value.to_string(),
                    })?
                    .to_string(),
            )),
            FieldType::Number => Ok(FieldValue::Number(value.as_float().ok_or(
                InvalidFieldError {
                    field: key.to_string(),
                    value: value.to_string(),
                },
            )?)),
            FieldType::Date => {
                let mut date_str = format!(
                    "{}",
                    value.as_str().ok_or(InvalidFieldError {
                        field: key.to_string(),
                        value: value.to_string(),
                    })?
                );
                // Also pretty lazy: check if we're missing time and add it
                if !date_str.contains(":") {
                    date_str = format!("{} 00:00:00", date_str);
                }
                // Supported formats:
                //
                // * `default` - `YYYY-MM-DD HH:MM:SS`
                // * `day_month` - `DD Month YYYY HH:MM:SS`
                // * `day_mon` - `DD Mon YYYY HH:MM:SS`
                // * `mdy` -  `MM/DD/YYYY HH:MM:SS`
                // * `dow_mon` - `Dow Mon DD HH:MM:SS YYYY`
                //
                // Offsets in one of the following forms, and are catenated with any of
                // the above formats.
                //
                // * `+HHMM`
                // * `-HHMM`
                let liquid_date =
                    model::DateTime::from_str(&date_str).ok_or(InvalidFieldError {
                        field: key.to_string(),
                        value: value.to_string(),
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
            _ => Err(Box::new(InvalidFieldError {
                field: key.to_string(),
                value: value.to_string(),
            })),
        }
    }

    fn to_string(&self) -> String {
        match self {
            FieldValue::String(s) => s.clone(),
            FieldValue::Number(n) => n.to_string(),
            FieldValue::Date(d) => d.to_rfc2822(),
            FieldValue::Objects(o) => format!("{:?}", o),
        }
    }
}

#[derive(Debug, ObjectView, ValueView, Deserialize, Serialize, Clone)]
pub struct Object {
    pub name: String,
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
                let field_value = FieldValue::from_toml(&key, field_type, &value)?;
                values.insert(key.to_string(), field_value);
            } else if let Some(child_def) = definition.children.get(&key.to_string()) {
                // Children
                let m_objects = value.as_array().ok_or(InvalidFieldError {
                    field: key.to_string(),
                    value: value.to_string(),
                })?;
                let mut objects: Vec<ObjectValues> = Vec::new();
                let mut index = 0;
                for object in m_objects {
                    let table = object.as_table().ok_or(InvalidFieldError {
                        field: format!("{}: {}", key, index),
                        value: value.to_string(),
                    })?;
                    let object = Object::values_from_table(&table, child_def)?;
                    objects.push(object);
                    index += 1;
                }
                let field_value = FieldValue::Objects(objects);
                values.insert(key.to_string(), field_value);
            } else if !is_reserved_field(&key) {
                println!("Unknown field {}", key);
            }
        }
        Ok(values)
    }

    pub fn from_table(
        definition: &ObjectDefinition,
        name: &String,
        table: &Table,
    ) -> Result<Object, Box<dyn Error>> {
        let values = Object::values_from_table(&table, definition)?;
        let mut order = -1;
        if let Some(t_order) = table.get(reserved_fields::ORDER) {
            if let Some(int_order) = t_order.as_integer() {
                order = int_order as i32;
            } else {
                println!("Invalid order {}", t_order);
            }
        }
        let object = Object {
            name: name.clone(),
            object_name: definition.name.clone(),
            order,
            values,
        };
        Ok(object)
    }
}

// Definitions

#[derive(Debug, Clone)]
struct InvalidFieldError {
    field: String,
    value: String,
}
impl Error for InvalidFieldError {}
impl fmt::Display for InvalidFieldError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid field {}: {}", self.field, self.value)
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
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
                value: "".to_string(),
            }),
        }
    }
}

pub type ObjectDefinitions = HashMap<String, ObjectDefinition>;

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
mod tests {
    use liquid::model::DateTime;

    use super::*;

    fn definition_str() -> &'static str {
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
    fn definition_parsing() -> Result<(), Box<dyn Error>> {
        let table: Table = toml::from_str(definition_str())?;
        let defs = ObjectDefinition::from_table(&table)?;

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

    #[test]
    fn object_parsing() -> Result<(), Box<dyn Error>> {
        let defs = ObjectDefinition::from_table(&toml::from_str(definition_str())?)?;
        let table: Table = toml::from_str(artist_object_str())?;
        let obj = Object::from_table(
            defs.get("artist").unwrap(),
            &"tormenta-rey".to_string(),
            &table,
        )?;
        assert_eq!(obj.order, 1);
        assert_eq!(obj.object_name, "artist");
        assert_eq!(obj.name, "tormenta-rey");
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
