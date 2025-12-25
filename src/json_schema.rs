use std::collections::HashSet;

use serde_json::json;
use time::Date;

use crate::{object::ValuePath, FieldType, ObjectDefinition, ObjectDefinitions};

pub type ObjectSchema = serde_json::Map<String, serde_json::Value>;

type DecoratorFn = dyn FnMut(&ValuePath, &FieldType, &mut ObjectSchema);
#[derive(Default)]
pub struct ObjectSchemaOptions {
    pub all_fields_required: bool,
    // If a "date" format isn't supported, this option allows setting them to a
    // static value.
    pub set_dates_to: Option<Date>,
    // Some generators don't support oneOf but do support anyOf
    pub anyof_for_unions: bool,
    pub name: Option<String>,
    // Decorator will be called for every leaf type, including oneof branches.
    // It is used to override the default output when needed.
    pub decorator_fn: Option<Box<DecoratorFn>>,
    pub omit_paths: Option<Vec<ValuePath>>,
}

impl ObjectSchemaOptions {
    pub fn with_decorator(
        mut self,
        decorator: impl FnMut(&ValuePath, &FieldType, &mut ObjectSchema) + 'static,
    ) -> Self {
        self.decorator_fn = Some(Box::new(decorator));
        self
    }
    pub fn with_anyof_for_unions(mut self) -> Self {
        self.anyof_for_unions = true;
        self
    }
    pub fn with_all_fields_required(mut self) -> Self {
        self.all_fields_required = true;
        self
    }
    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }
    pub fn with_date(mut self, date: Date) -> Self {
        self.set_dates_to = Some(date);
        self
    }
    pub fn with_omit_paths(mut self, paths: Option<Vec<ValuePath>>) -> Self {
        self.omit_paths = paths;
        self
    }

    pub(crate) fn decorate(
        &mut self,
        field_path: &ValuePath,
        field_type: &FieldType,
        schema: &mut ObjectSchema,
    ) {
        if let Some(decorator) = &mut self.decorator_fn {
            decorator(field_path, field_type, schema);
        }
    }
}

impl std::fmt::Debug for ObjectSchemaOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectSchemaOptions")
            .field("all_fields_required", &self.all_fields_required)
            .field("set_dates_to", &self.set_dates_to)
            .field("anyof_for_unions", &self.anyof_for_unions)
            .field("name", &self.name)
            .field("decorator_fn", &self.decorator_fn.is_some())
            .field("omit_paths", &self.omit_paths)
            .finish()
    }
}

pub fn generate_root_json_schema(
    id: &str,
    title: Option<&str>,
    description: &str,
    objects: &ObjectDefinitions,
    root_objects: &HashSet<String>,
    mut options: ObjectSchemaOptions,
) -> ObjectSchema {
    let mut schema = serde_json::Map::new();
    schema.insert("$id".into(), id.into());
    if let Some(title) = title {
        schema.insert("title".into(), title.into());
    }
    schema.insert("description".into(), description.into());
    schema.insert("type".into(), "object".into());
    schema.insert("additionalProperties".into(), false.into());
    let mut properties = serde_json::Map::new();
    for (name, def) in objects {
        let object_path = ValuePath::empty().append(ValuePath::key(name));
        // Skip if this object is omitted
        if options
            .omit_paths
            .as_ref()
            .is_some_and(|op| op.contains(&object_path))
        {
            continue;
        }
        let obj_properties = def.to_json_schema_properties(false, &mut options, object_path);
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
    // name: &str,
    // description: &str,
    definition: &ObjectDefinition,
    mut options: ObjectSchemaOptions,
) -> ObjectSchema {
    let mut schema = serde_json::Map::new();
    schema.insert("$id".into(), id.into());
    // schema.insert("title".into(), name.into());
    // schema.insert("description".into(), description.into());
    schema.insert("type".into(), "object".into());
    let object_path = ValuePath::empty();
    let properties = definition.to_json_schema_properties(false, &mut options, object_path);
    if options.all_fields_required {
        let keys: Vec<String> = properties.keys().map(|k| k.to_string()).collect();
        schema.insert("required".into(), keys.into());
    }
    schema.insert("properties".into(), properties.into());
    schema.insert("additionalProperties".into(), false.into());
    schema
}

#[cfg(test)]
pub mod tests {

    use ordermap::OrderMap;
    use serde_json::json;
    use std::{collections::HashSet, error::Error};
    use toml::Table;

    use crate::{
        json_schema::{generate_json_schema, generate_root_json_schema, ObjectSchemaOptions},
        object::ValuePath,
        ObjectDefinition,
    };

    pub fn artist_and_example_definition_str() -> &'static str {
        r#"[artists]
        name = "string"
        meta = "meta"
        genre = ["emo","metal"]
        template = "artist"
        [artists.tour_dates]
        date = "date"
        ticket_link = "string"
        [artists.videos]
        video = "video"
        [artists.numbers]
        number = "number"
        
        [example]
        content = "markdown"
        [example.links]
        url = "string"
        [example.children]
        [example.children.omit_me]
        foo = "string"
        [example.omitted]
        foo = "string"

        [omitted]
        foo = "string"
        "#
    }

    #[test]
    fn json_schema_generation() -> Result<(), Box<dyn Error>> {
        let table: Table = toml::from_str(artist_and_example_definition_str())?;
        let defs = ObjectDefinition::from_table(&table, &OrderMap::new())?;

        let schema = generate_json_schema(
            "artists",
            defs.get("artists").unwrap(),
            ObjectSchemaOptions::default(),
        );
        println!("SCHEMA: {:#?}", schema);
        let instance = json!({
            "genre": "emo",
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
        Ok(())
    }

    #[test]
    fn omitted_fields() -> Result<(), Box<dyn Error>> {
        let table: Table = toml::from_str(artist_and_example_definition_str())?;
        let defs = ObjectDefinition::from_table(&table, &OrderMap::new())?;

        let schema = generate_json_schema(
            "example",
            defs.get("example").unwrap(),
            ObjectSchemaOptions::default().with_omit_paths(Some(vec![
                ValuePath::from_string("omitted"),
                ValuePath::from_string("child.omit_me"),
            ])),
        );
        println!("SCHEMA: {:#?}", schema);
        let instance = json!({
            "children": []
        });

        let schema_value = &schema.into();
        assert!(jsonschema::validate(
            schema_value,
            &json!({
                "omitted": { "foo": "bar" }
            })
        )
        .is_err());
        assert!(jsonschema::validate(
            schema_value,
            &json!({
                "child": { "omitted": {"foo": "bar"} }
            })
        )
        .is_err());
        assert!(jsonschema::is_valid(schema_value, &instance));
        Ok(())
    }

    #[test]
    fn root_omitted_fields() -> Result<(), Box<dyn Error>> {
        let table: Table = toml::from_str(artist_and_example_definition_str())?;
        let defs = ObjectDefinition::from_table(&table, &OrderMap::new())?;

        let options = ObjectSchemaOptions::default()
            .with_omit_paths(Some(vec![ValuePath::from_string("omitted")]));
        let root_objects = defs.keys().cloned().collect::<HashSet<String>>();
        let schema = generate_root_json_schema(
            "id",
            Some("title"),
            "description",
            &defs,
            &root_objects,
            options,
        );
        println!("SCHEMA: {:#?}", schema);
        let instance = json!({});

        let schema_value = &schema.into();
        assert!(jsonschema::validate(
            schema_value,
            &json!({
                "omitted": { "foo": "bar" }
            })
        )
        .is_err());
        assert!(jsonschema::is_valid(schema_value, &instance));
        Ok(())
    }
}
