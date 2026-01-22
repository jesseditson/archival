use indefinite::indefinite;
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fmt::{Debug, Display},
    hash::Hash,
};
#[cfg(feature = "typescript")]
use typescript_type_def::TypeDef;

use crate::{
    object::Object, util::integer_decode, value_path::ValuePath, Archival, FieldValue,
    FileSystemAPI,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub enum ArchivalEvent {
    RenameObject(RenameObjectEvent),
    AddObject(AddObjectEvent),
    AddRootObject(AddRootObjectEvent),
    DeleteObject(DeleteObjectEvent),
    EditField(EditFieldEvent),
    EditOrder(EditOrderEvent),
    AddChild(AddChildEvent),
    RemoveChild(RemoveChildEvent),
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
            ArchivalEvent::RenameObject(evt) => &evt.object,
        }
    }
    /// Get the current filename of a given event. Note that for renames, this
    /// will return the current filename, not the name it was renamed from, as
    /// this method is usually used to find an (existing) file.
    pub fn filename(&self) -> &str {
        match self {
            ArchivalEvent::AddObject(evt) => &evt.filename,
            ArchivalEvent::AddRootObject(evt) => &evt.object,
            ArchivalEvent::DeleteObject(evt) => &evt.filename,
            ArchivalEvent::EditField(evt) => &evt.filename,
            ArchivalEvent::EditOrder(evt) => &evt.filename,
            ArchivalEvent::AddChild(evt) => &evt.filename,
            ArchivalEvent::RemoveChild(evt) => &evt.filename,
            ArchivalEvent::RenameObject(evt) => &evt.to,
        }
    }
}

impl ArchivalEvent {
    pub fn content<F>(&self, archival: &Archival<F>) -> Result<String, Box<dyn Error>>
    where
        F: FileSystemAPI + Clone + Debug,
    {
        match self {
            ArchivalEvent::DeleteObject(_) => {
                archival.object_file(self.object_name(), self.filename())
            }
            ArchivalEvent::RenameObject(evt) => archival.object_file(&evt.object, &evt.from),
            ArchivalEvent::AddObject(evt) => {
                let obj_def = archival.get_object_definition(&evt.object)?;
                let object =
                    Object::from_def(obj_def, &evt.filename, evt.order, evt.values.clone())?;
                Ok(object.to_toml(obj_def)?)
            }
            evt => archival.object_file(evt.object_name(), evt.filename()),
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
                    let child_name = &evt
                        .path
                        .first()
                        .expect("AddChild has no object")
                        .to_string();
                    format!(
                        "Add {} child to {} '{}'",
                        indefinite(child_name),
                        evt.object,
                        evt.filename
                    )
                }
                ArchivalEvent::RemoveChild(evt) => {
                    let child_name = &evt
                        .path
                        .first()
                        .expect("RemoveChild has no object")
                        .to_string();
                    format!(
                        "Remove {} child from {} '{}'",
                        indefinite(child_name),
                        evt.object,
                        evt.filename
                    )
                }
                ArchivalEvent::RenameObject(evt) => {
                    format!(
                        "Rename {} '{}' to '{}'",
                        indefinite(&evt.object),
                        evt.from,
                        evt.to
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct EditFieldEvent {
    pub object: String,
    pub filename: String,
    pub path: ValuePath,
    pub field: String,
    pub value: Option<FieldValue>,
    pub source: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct EditOrderEvent {
    pub object: String,
    pub filename: String,
    pub order: Option<f64>,
    pub source: Option<String>,
}

impl Hash for EditOrderEvent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.object.hash(state);
        self.filename.hash(state);
        self.order.map(integer_decode).hash(state);
        self.source.hash(state);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct DeleteObjectEvent {
    pub object: String,
    pub filename: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct AddObjectValue {
    pub path: ValuePath,
    pub value: FieldValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct AddObjectEvent {
    pub object: String,
    pub filename: String,
    pub order: Option<f64>,
    pub values: Vec<AddObjectValue>,
}

impl Hash for AddObjectEvent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.object.hash(state);
        self.filename.hash(state);
        self.order.map(integer_decode).hash(state);
        self.values.hash(state);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct AddRootObjectEvent {
    pub object: String,
    pub values: Vec<AddObjectValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct AddChildEvent {
    pub object: String,
    pub filename: String,
    pub path: ValuePath,
    pub values: Vec<AddObjectValue>,
    /// If not provided, this will just append to the end of the child list.
    pub index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct RemoveChildEvent {
    pub object: String,
    pub filename: String,
    pub path: ValuePath,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct RenameObjectEvent {
    pub object: String,
    pub from: String,
    pub to: String,
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
