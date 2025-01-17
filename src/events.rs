use indefinite::indefinite;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
#[cfg(feature = "typescript")]
use typescript_type_def::TypeDef;

use crate::{value_path::ValuePath, FieldValue};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub enum ArchivalEvent {
    AddObject(AddObjectEvent),
    AddRootObject(AddRootObjectEvent),
    DeleteObject(DeleteObjectEvent),
    EditField(EditFieldEvent),
    EditOrder(EditOrderEvent),
    AddChild(ChildEvent),
    RemoveChild(ChildEvent),
}

impl ArchivalEvent {
    pub fn object_name(&self) -> &str {
        match self {
            ArchivalEvent::AddObject(evt) => &evt.object,
            ArchivalEvent::AddRootObject(evt) => &evt.object,
            ArchivalEvent::DeleteObject(evt) => &evt.object,
            ArchivalEvent::EditField(evt) => &evt.object,
            ArchivalEvent::EditOrder(evt) => &evt.object,
            ArchivalEvent::AddChild(evt) => &evt.object,
            ArchivalEvent::RemoveChild(evt) => &evt.object,
        }
    }
    pub fn filename(&self) -> &str {
        match self {
            ArchivalEvent::AddObject(evt) => &evt.filename,
            ArchivalEvent::AddRootObject(evt) => &evt.object,
            ArchivalEvent::DeleteObject(evt) => &evt.filename,
            ArchivalEvent::EditField(evt) => &evt.filename,
            ArchivalEvent::EditOrder(evt) => &evt.filename,
            ArchivalEvent::AddChild(evt) => &evt.filename,
            ArchivalEvent::RemoveChild(evt) => &evt.filename,
        }
    }
}

impl Display for ArchivalEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ArchivalEvent::AddObject(evt) => {
                    format!("Add {} '{}'", indefinite(&evt.object), evt.filename)
                }
                ArchivalEvent::AddRootObject(evt) => {
                    format!("Add {}", evt.object)
                }
                ArchivalEvent::DeleteObject(evt) =>
                    format!("Delete {} '{}'", evt.object, evt.filename),
                ArchivalEvent::EditField(evt) => format!(
                    "Change field {} in {} '{}'",
                    evt.field, evt.object, evt.filename
                ),
                ArchivalEvent::EditOrder(evt) => {
                    format!("Update order of {} '{}'", evt.object, evt.filename)
                }
                ArchivalEvent::AddChild(evt) => {
                    let child_name = evt.path.child_name().unwrap();
                    format!(
                        "Add {} child to {} '{}'",
                        indefinite(child_name),
                        evt.object,
                        evt.filename
                    )
                }
                ArchivalEvent::RemoveChild(evt) => {
                    let child_name = evt.path.child_name().unwrap();
                    format!(
                        "Remove {} child from {} '{}'",
                        indefinite(child_name),
                        evt.object,
                        evt.filename
                    )
                }
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub enum ArchivalEventResponse {
    None,
    Index(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct EditFieldEvent {
    pub object: String,
    pub filename: String,
    pub path: ValuePath,
    pub field: String,
    pub value: Option<FieldValue>,
    pub source: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct EditOrderEvent {
    pub object: String,
    pub filename: String,
    pub order: i32,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct DeleteObjectEvent {
    pub object: String,
    pub filename: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct AddObjectValue {
    pub path: ValuePath,
    pub value: FieldValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct AddObjectEvent {
    pub object: String,
    pub filename: String,
    pub order: i32,
    pub values: Vec<AddObjectValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct AddRootObjectEvent {
    pub object: String,
    pub values: Vec<AddObjectValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct ChildEvent {
    pub object: String,
    pub filename: String,
    pub path: ValuePath,
    pub source: Option<String>,
}

#[cfg(test)]
#[cfg(feature = "typescript")]
mod export_types {
    use std::fs;
    use typescript_type_def::{write_definition_file, DefinitionFileOptions};

    use super::*;

    #[test]
    fn run() {
        let mut buf = Vec::new();
        let options = DefinitionFileOptions {
            header: Some("// AUTO-GENERATED by typescript-type-def\n"),
            root_namespace: None,
        };
        write_definition_file::<_, ArchivalEvent>(&mut buf, options).unwrap();
        fs::write("./events.d.ts", buf).expect("Failed to write file");
    }
}
