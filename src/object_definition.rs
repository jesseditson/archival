#[cfg(feature = "json-schema")]
use crate::object::ValuePath;
use crate::{
    fields::{field_type::InvalidFieldError, FieldType, ObjectValues, OneofOption},
    manifest::EditorTypes,
    reserved_fields::{self, is_reserved_field, reserved_field_from_str, ReservedFieldError},
    FieldValue,
};
use ordermap::OrderMap;
use serde::{Deserialize, Serialize};
#[cfg(feature = "json-schema")]
use serde_json::json;
use std::{error::Error, fmt::Debug};
use toml::Table;
use tracing::instrument;

pub type ObjectDefinitions = OrderMap<String, ObjectDefinition>;
#[cfg(feature = "typescript")]
pub mod typedefs {
    use typescript_type_def::{
        type_expr::{Ident, NativeTypeInfo, TypeExpr, TypeInfo, TypeName},
        TypeDef,
    };

    use crate::{fields::FieldType, ObjectDefinition};
    pub struct ObjectDefinitionsDef;
    impl TypeDef for ObjectDefinitionsDef {
        const INFO: TypeInfo = TypeInfo::Native(NativeTypeInfo {
            r#ref: TypeExpr::Name(TypeName {
                path: &[],
                name: Ident("Record"),
                generic_args: &[
                    TypeExpr::Ref(&String::INFO),
                    TypeExpr::Ref(&ObjectDefinition::INFO),
                ],
            }),
        });
    }
    pub struct FieldsMapDef;
    impl TypeDef for FieldsMapDef {
        const INFO: TypeInfo = TypeInfo::Native(NativeTypeInfo {
            r#ref: TypeExpr::Name(TypeName {
                path: &[],
                name: Ident("Record"),
                generic_args: &[
                    TypeExpr::Ref(&String::INFO),
                    TypeExpr::Ref(&FieldType::INFO),
                ],
            }),
        });
    }

    pub struct ChildrenDef;
    impl TypeDef for ChildrenDef {
        const INFO: TypeInfo = TypeInfo::Native(NativeTypeInfo {
            r#ref: TypeExpr::ident(Ident("Record<string, ObjectDefinition>")),
        });
    }
}

pub type FieldsMap = OrderMap<String, FieldType>;

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Hash)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct ObjectDefinition {
    pub name: String,
    #[cfg_attr(feature = "typescript", type_def(type_of = "typedefs::FieldsMapDef"))]
    pub fields: FieldsMap,
    pub template: Option<String>,

    #[cfg_attr(feature = "typescript", type_def(type_of = "typedefs::ChildrenDef"))]
    pub children: ObjectDefinitions,
}

impl ObjectDefinition {
    pub fn new(
        name: &str,
        definition: &Table,
        editor_types: &EditorTypes,
    ) -> Result<ObjectDefinition, Box<dyn Error>> {
        if is_reserved_field(name) {
            return Err(InvalidFieldError::ReservedObjectNameError(name.to_string()).into());
        }
        let mut obj_def = ObjectDefinition {
            name: name.to_string(),
            fields: FieldsMap::new(),
            template: None,
            children: ObjectDefinitions::new(),
        };
        for (key, m_value) in definition {
            if is_reserved_field(key)
                && !(m_value.as_str().is_some() && key == reserved_fields::TEMPLATE)
            {
                return Err(Box::new(ReservedFieldError {
                    field: reserved_field_from_str(key),
                }));
            }
            if let Some(child_table) = m_value.as_table() {
                obj_def.children.insert(
                    key.clone(),
                    ObjectDefinition::new(key, child_table, editor_types)?,
                );
            } else if let Some(value) = m_value.as_array() {
                if let Some(tables) = value
                    .iter()
                    .map(|v| v.as_table())
                    .collect::<Option<Vec<_>>>()
                {
                    // this is an array of tables, try to parse as oneof
                    if let Some(options) = tables
                        .iter()
                        .map(|info| {
                            let name = info.get("name")?.as_str()?;
                            let type_name = info.get("type")?.as_str()?;
                            let def_type = FieldType::from_str(type_name, editor_types).ok()?;
                            Some((name, def_type))
                        })
                        .collect::<Option<Vec<_>>>()
                    {
                        let options = options
                            .into_iter()
                            .map(|(name, def_type)| OneofOption {
                                name: name.to_string(),
                                r#type: def_type,
                            })
                            .collect();
                        obj_def
                            .fields
                            .insert(key.clone(), FieldType::Oneof(options));
                    } else {
                        return Err(Box::new(InvalidFieldError::InvalidOneof(format!(
                            "{tables:?}"
                        ))));
                    }
                } else {
                    // not objects, try to parse as enum
                    let string_vals = value
                        .iter()
                        .map(|v| {
                            v.as_str()
                                .map(|s| s.to_string())
                                .ok_or_else(|| InvalidFieldError::InvalidEnum(format!("{value:?}")))
                        })
                        .collect::<Result<Vec<String>, InvalidFieldError>>()?;
                    obj_def
                        .fields
                        .insert(key.clone(), FieldType::Enum(string_vals));
                }
            } else if let Some(value) = m_value.as_str() {
                if key == reserved_fields::TEMPLATE {
                    obj_def.template = Some(value.to_string());
                } else {
                    obj_def
                        .fields
                        .insert(key.clone(), FieldType::from_str(value, editor_types)?);
                }
            }
        }
        Ok(obj_def)
    }
    pub fn from_table(
        table: &Table,
        editor_types: &EditorTypes,
    ) -> Result<ObjectDefinitions, Box<dyn Error>> {
        let mut objects = ObjectDefinitions::new();
        for (name, m_def) in table.into_iter() {
            if let Some(def) = m_def.as_table() {
                objects.insert(
                    name.clone(),
                    ObjectDefinition::new(name, def, editor_types)?,
                );
            }
        }
        Ok(objects)
    }

    #[instrument(skip(self))]
    pub fn empty_object(&self) -> ObjectValues {
        let mut values: ObjectValues = ObjectValues::new();
        for def in self.children.values() {
            values.insert(def.name.to_owned(), FieldValue::Objects(vec![]));
        }
        values
    }
}

#[cfg(feature = "json-schema")]
impl ObjectDefinition {
    pub fn to_json_schema_properties(
        &self,
        is_child: bool,
        options: &mut crate::json_schema::ObjectSchemaOptions,
        current_path: ValuePath,
    ) -> crate::json_schema::ObjectSchema {
        let mut properties = serde_json::Map::new();
        for (field, field_type) in &self.fields {
            let field_path = current_path.clone().append(ValuePath::key(field));
            // Skip if this path is omitted
            if options
                .omit_paths
                .as_ref()
                .is_some_and(|op| op.contains(&field_path))
            {
                continue;
            }
            let field_props = field_type.to_json_schema_property(field, &field_path, options);
            properties.insert(field.into(), field_props.into());
        }
        if !is_child {
            properties.insert("order".into(), json!({"type": "number"}));
        }
        for (name, definition) in &self.children {
            let child_path = current_path.clone().append(ValuePath::key(name));
            // Skip if this path is omitted
            if options
                .omit_paths
                .as_ref()
                .is_some_and(|op| op.contains(&child_path))
            {
                continue;
            }
            let mut child = serde_json::Map::new();
            child.insert("description".into(), name.to_string().into());
            child.insert("type".into(), "array".into());
            let child_properties = definition.to_json_schema_properties(true, options, child_path);
            let mut child_items_type = serde_json::Map::new();
            child_items_type.insert("type".into(), "object".into());
            child_items_type.insert("additionalProperties".into(), false.into());
            if options.all_fields_required {
                let keys: Vec<String> = child_properties.keys().map(|k| k.to_string()).collect();
                child_items_type.insert("required".into(), keys.into());
            }
            child_items_type.insert("properties".into(), child_properties.into());
            child.insert("items".into(), child_items_type.into());
            properties.insert(name.into(), child.into());
        }
        properties
    }
}

#[cfg(test)]
pub mod tests {

    use super::*;

    pub fn artist_and_example_definition_str() -> &'static str {
        r#"
        [artists]
        name = "string"
        meta = "meta"
        genre = ["emo", "metal"]
        template = "artist"
        [[artists.media]]
        name = "video"
        type = "video"
        [[artists.media]]
        name = "photo"
        type = "image"
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
        "#
    }

    #[test]
    fn parsing() {
        let table: Table = toml::from_str(artist_and_example_definition_str()).unwrap();
        let defs = ObjectDefinition::from_table(&table, &OrderMap::new()).unwrap();

        println!("{:?}", defs);

        assert_eq!(defs.keys().len(), 2);
        assert!(defs.contains_key("artists"));
        assert!(defs.contains_key("example"));
        let artist = defs.get("artists").unwrap();
        let fields = artist.fields.keys();
        assert_eq!(fields.len(), 4);
        assert_eq!(fields[0], "name".to_string());
        assert_eq!(fields[1], "meta".to_string());
        assert_eq!(fields[2], "genre".to_string());
        assert_eq!(fields[3], "media".to_string());
        let oneof_field = artist.fields.get("media").unwrap();
        assert!(matches!(oneof_field, FieldType::Oneof(_)));
        if let FieldType::Oneof(field) = oneof_field {
            assert_eq!(field.len(), 2);
            assert_eq!(field[0].name, "video");
            assert_eq!(field[1].name, "photo");
            assert!(matches!(field[0].r#type, FieldType::Video));
            assert!(matches!(field[1].r#type, FieldType::Image));
        }
        let children = artist.children.keys();
        assert_eq!(children.len(), 3);
        assert_eq!(children[0], "tour_dates".to_string());
        assert_eq!(children[1], "videos".to_string());
        assert_eq!(children[2], "numbers".to_string());
        assert!(artist.fields.contains_key("name"));
        assert_eq!(artist.fields.get("name").unwrap(), &FieldType::String);
        assert!(
            !artist.fields.contains_key("template"),
            "did not copy the template reserved field"
        );
        assert!(artist.fields.contains_key("name"));
        assert_eq!(
            artist.fields.get("genre").unwrap(),
            &FieldType::Enum(vec!["emo".to_string(), "metal".to_string()])
        );
        assert!(artist.template.is_some());
        assert_eq!(artist.template, Some("artist".to_string()));
        assert_eq!(artist.children.len(), 3);
        assert!(artist.children.contains_key("tour_dates"));
        assert!(artist.children.contains_key("numbers"));
        let tour_dates = artist.children.get("tour_dates").unwrap();
        assert!(tour_dates.fields.contains_key("date"));
        assert_eq!(tour_dates.fields.get("date").unwrap(), &FieldType::Date);
        assert!(tour_dates.fields.contains_key("ticket_link"));
        assert_eq!(
            tour_dates.fields.get("ticket_link").unwrap(),
            &FieldType::String
        );
        let numbers = artist.children.get("numbers").unwrap();
        assert!(numbers.fields.contains_key("number"));
        assert_eq!(numbers.fields.get("number").unwrap(), &FieldType::Number);
        let numbers = artist.children.get("videos").unwrap();
        assert!(numbers.fields.contains_key("video"));
        assert_eq!(numbers.fields.get("video").unwrap(), &FieldType::Video);

        let example = defs.get("example").unwrap();
        assert!(example.fields.contains_key("content"));
        assert_eq!(example.fields.get("content").unwrap(), &FieldType::Markdown);
        assert_eq!(example.children.len(), 1);
        assert!(example.children.contains_key("links"));
        let links = example.children.get("links").unwrap();
        assert!(links.fields.contains_key("url"));
        assert_eq!(links.fields.get("url").unwrap(), &FieldType::String);
    }
}
