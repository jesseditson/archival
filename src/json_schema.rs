use std::collections::{BTreeMap, HashSet};

use serde_json::json;

use crate::ObjectDefinition;

pub type ObjectSchema = serde_json::Map<String, serde_json::Value>;

#[derive(Debug, Default)]
pub struct ObjectSchemaOptions {
    pub omit_file_types: bool,
    pub all_fields_required: bool,
    pub name: Option<String>,
}

impl ObjectSchemaOptions {
    pub fn open_ai_compatible(name: Option<String>) -> Self {
        Self {
            omit_file_types: true,
            all_fields_required: true,
            name,
        }
    }
}

pub fn generate_root_json_schema(
    id: &str,
    title: Option<&str>,
    description: &str,
    objects: &BTreeMap<String, ObjectDefinition>,
    root_objects: &HashSet<String>,
    options: ObjectSchemaOptions,
) -> ObjectSchema {
    let mut schema = serde_json::Map::new();
    schema.insert(
        "$schema".into(),
        "https://json-schema.org/draft/2020-12/schema".into(),
    );
    schema.insert("$id".into(), id.into());
    if let Some(title) = title {
        schema.insert("title".into(), title.into());
    }
    schema.insert("description".into(), description.into());
    schema.insert("type".into(), "object".into());
    schema.insert("additionalProperties".into(), false.into());
    let mut properties = serde_json::Map::new();
    for (name, def) in objects {
        let obj_properties = def.to_json_schema_properties(false, &options);
        let required: Vec<String> = if options.all_fields_required {
            obj_properties.keys().map(|k| k.to_string()).collect()
        } else {
            vec![]
        };
        if root_objects.contains(name) {
            properties.insert(
                name.into(),
                json!({
                    "type": "object",
                    "$comment": "root object",
                    "description": name,
                    "properties": obj_properties,
                    "required": required,
                    "additionalProperties": false,
                }),
            );
        } else {
            properties.insert(
                name.into(),
                json!({
                    "type": "array",
                    "description": name,
                    "items": {
                        "type": "object",
                        "properties": obj_properties,
                        "required": required,
                        "additionalProperties": false,
                    }
                }),
            );
        }
    }
    if options.all_fields_required {
        let keys: Vec<String> = properties.keys().map(|k| k.to_string()).collect();
        schema.insert("required".into(), keys.into());
    }
    schema.insert("properties".into(), properties.into());
    schema
}

pub fn generate_json_schema(
    id: &str,
    // title: &str,
    // description: &str,
    definition: &ObjectDefinition,
    options: crate::json_schema::ObjectSchemaOptions,
) -> ObjectSchema {
    let mut schema = serde_json::Map::new();
    schema.insert(
        "$schema".into(),
        "https://json-schema.org/draft/2020-12/schema".into(),
    );
    schema.insert("$id".into(), id.into());
    // schema.insert("title".into(), title.into());
    // schema.insert("description".into(), description.into());
    schema.insert("type".into(), "object".into());
    let properties = definition.to_json_schema_properties(false, &options);
    if options.all_fields_required {
        let keys: Vec<String> = properties.keys().map(|k| k.to_string()).collect();
        schema.insert("required".into(), keys.into());
    }
    schema.insert("properties".into(), properties.into());
    schema
}

#[cfg(test)]
pub mod tests {

    use serde_json::json;
    use std::{collections::HashMap, error::Error};
    use toml::Table;

    use crate::{
        json_schema::{generate_json_schema, ObjectSchemaOptions},
        ObjectDefinition,
    };

    pub fn artist_and_example_definition_str() -> &'static str {
        "[artist]
        name = \"string\"
        meta = \"meta\"
        template = \"artist\"
        [artist.tour_dates]
        date = \"date\"
        ticket_link = \"string\"
        [artist.videos]
        video = \"video\"
        [artist.numbers]
        number = \"number\"
        
        [example]
        content = \"markdown\"
        [example.links]
        url = \"string\""
    }

    #[test]
    fn json_schema_generation() -> Result<(), Box<dyn Error>> {
        let table: Table = toml::from_str(artist_and_example_definition_str())?;
        let defs = ObjectDefinition::from_table(&table, &HashMap::new())?;

        let schema = generate_json_schema(
            "artist",
            defs.get("artist").unwrap(),
            ObjectSchemaOptions::default(),
        );
        println!("SCHEMA: {:#?}", schema);
        let instance = json!({
            "tour_dates": [{
                "date": "2021-01-26 00:01:22",
                "ticket_link": "https://archival.dev"
            }],
            "videos": [
                {"video": {
                    "sha": "12e90b8e74f20fc0a7274cff9fcbae14592db12292757f1ea0d7503d30799fd2",
                    "filename": "butts.mp4",
                    "mime": "video/mp4",
                    "display_type": "video"
                }},
            ],
            "numbers": [{"number": 44}, {"number": 7.2}],
        });

        let schema_value = &schema.into();
        assert!(jsonschema::is_valid(schema_value, &instance));
        assert!(jsonschema::validate(schema_value, &json!("Hello, world!")).is_err());
        assert!(jsonschema::validate(schema_value, &instance).is_ok());

        Ok(())
    }
}
