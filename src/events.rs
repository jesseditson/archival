use serde::{Deserialize, Serialize};
#[cfg(feature = "typescript")]
use typescript_type_def::TypeDef;

use crate::object::ValuePath;

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub enum ArchivalEvent {
    AddObject(AddObjectEvent),
    EditField(EditFieldEvent),
    EditOrder(EditOrderEvent),
}

#[derive(Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub enum EditFieldValue {
    String(String),
    Markdown(String),
    Number(f64),
    Date(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct EditFieldEvent {
    pub object: String,
    pub filename: String,
    pub path: ValuePath,
    pub value: EditFieldValue,
}
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct EditOrderEvent {
    pub object: String,
    pub filename: String,
    pub order: i32,
}
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(TypeDef))]
pub struct AddObjectEvent {
    pub object: String,
    pub filename: String,
    pub order: i32,
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
