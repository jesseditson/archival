use std::collections::{HashMap, HashSet};

use serde_json::json;

use crate::ObjectDefinition;

pub fn generate_root_json_schema(
    id: &str,
    title: Option<&str>,
    description: &str,
    objects: &HashMap<String, ObjectDefinition>,
    root_objects: &HashSet<String>,
    pretty: bool,
) -> String {
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
    let mut properties = serde_json::Map::new();
    for (name, def) in objects {
        if root_objects.contains(name) {
            properties.insert(
                name.into(),
                json!({
                    "type": "object",
                    "$comment": "root object",
                    "description": name,
                    "properties": def.to_json_schema_properties()
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
                        "properties": def.to_json_schema_properties()
                    }
                }),
            );
        }
    }
    schema.insert("properties".into(), properties.into());
    if pretty {
        serde_json::to_string_pretty(&schema).unwrap()
    } else {
        serde_json::to_string(&schema).unwrap()
    }
}

pub fn generate_json_schema(
    id: &str,
    // title: &str,
    // description: &str,
    definition: &ObjectDefinition,
    pretty: bool,
) -> String {
    let mut schema = serde_json::Map::new();
    schema.insert(
        "$schema".into(),
        "https://json-schema.org/draft/2020-12/schema".into(),
    );
    schema.insert("$id".into(), id.into());
    // schema.insert("title".into(), title.into());
    // schema.insert("description".into(), description.into());
    schema.insert("type".into(), "object".into());
    schema.insert(
        "properties".into(),
        definition.to_json_schema_properties().into(),
    );
    if pretty {
        serde_json::to_string_pretty(&schema).unwrap()
    } else {
        serde_json::to_string(&schema).unwrap()
    }
}

#[cfg(test)]
pub mod tests {

    use serde_json::json;
    use std::{collections::HashMap, error::Error};
    use toml::Table;

    use crate::{json_schema::generate_json_schema, ObjectDefinition};

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

        let schema_str = generate_json_schema("artist", defs.get("artist").unwrap(), false);
        println!("SCHEMA: {}", schema_str);
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
        let schema = serde_json::from_str(&schema_str)?;

        assert!(jsonschema::is_valid(&schema, &instance));
        assert!(jsonschema::validate(&schema, &json!("Hello, world!")).is_err());
        assert!(jsonschema::validate(&schema, &instance).is_ok());

        Ok(())
    }
}
