use crate::{
    field_value::{self, FieldValue},
    object::Object,
    object_definition::FieldType,
    ObjectDefinition,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValuePathError {
    #[error("Child definition not found for path {0} in {1}")]
    ChildDefNotFound(String, String),
    #[error("Path {0} was not a children type in {1}")]
    NotChildren(String, String),
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub enum ValuePathComponent {
    Key(String),
    Index(usize),
}

impl ValuePathComponent {
    pub fn key(name: &str) -> Self {
        Self::Key(name.to_string())
    }
}

impl Display for ValuePathComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValuePathComponent::Index(i) => write!(f, "{}", i),
            ValuePathComponent::Key(k) => write!(f, "{}", k),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct ValuePath {
    path: Vec<ValuePathComponent>,
}

impl Default for ValuePath {
    fn default() -> Self {
        Self { path: vec![] }
    }
}

impl Display for ValuePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.path
                .iter()
                .map(|p| format!("{}", p))
                .collect::<Vec<String>>()
                .join(":")
        )
    }
}

impl ValuePath {
    pub fn join(mut self, component: ValuePathComponent) -> Self {
        self.path.push(component);
        self
    }

    pub fn get_in_object<'a>(&self, object: &'a Object) -> Option<&'a FieldValue> {
        let mut i_path = self.path.iter().map(|v| match v {
            ValuePathComponent::Index(i) => ValuePathComponent::Index(*i),
            ValuePathComponent::Key(k) => ValuePathComponent::Key(k.to_owned()),
        });
        let mut last_val = None;
        while let Some(cmp) = i_path.next() {
            if last_val.is_none() {
                // At the root, we must have a key string
                if let ValuePathComponent::Key(k) = cmp {
                    last_val = object.values.get(&k);
                    continue;
                }
            } else {
                // more than one level deep. We only allow accessing child
                // values, not children themselves - so this finds a child at
                // the index and then finds a key on it.
                if let Some(FieldValue::Objects(children)) = last_val {
                    if let ValuePathComponent::Index(index) = cmp {
                        if let Some(child) = children.get(index) {
                            if let Some(ValuePathComponent::Key(k)) = i_path.next() {
                                last_val = child.get(&k);
                                continue;
                            }
                        }
                    }
                }
            }
            break;
        }
        last_val
    }

    pub fn get_child_definition<'a>(
        &self,
        def: &'a ObjectDefinition,
    ) -> Result<&'a HashMap<String, FieldType>, ValuePathError> {
        let mut i_path = self.path.iter();
        let mut last_val = def;
        while let Some(cmp) = i_path.next() {
            if let ValuePathComponent::Key(k) = cmp {
                if let Some(child_def) = last_val.children.get(k) {
                    last_val = child_def;
                    continue;
                }
            }
            return Err(ValuePathError::ChildDefNotFound(
                self.to_string(),
                format!("{:?}", def),
            ));
        }
        Ok(&last_val.fields)
    }

    pub fn add_child<'a>(
        &self,
        object: &mut Object,
        obj_def: &'a ObjectDefinition,
    ) -> Result<(), ValuePathError> {
        let child_def = self.get_child_definition(obj_def)?;
        let new_child = field_value::def_to_values(child_def);
        let mut i_path = self.path.iter().map(|v| match v {
            ValuePathComponent::Index(i) => ValuePathComponent::Index(*i),
            ValuePathComponent::Key(k) => ValuePathComponent::Key(k.to_owned()),
        });
        let mut last_val = None;
        while let Some(cmp) = i_path.next() {
            if last_val.is_none() {
                // At the root, we must have a key string
                if let ValuePathComponent::Key(k) = cmp {
                    last_val = object.values.get_mut(&k);
                    continue;
                }
            } else {
                // more than one level deep. We only can recurse if there is an
                // objects value type.
                if let Some(FieldValue::Objects(children)) = last_val {
                    if let ValuePathComponent::Index(index) = cmp {
                        if let Some(child) = children.get_mut(index) {
                            if let Some(ValuePathComponent::Key(k)) = i_path.next() {
                                if i_path.len() > 0 {
                                    last_val = child.get_mut(&k);
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
            return Err(ValuePathError::NotChildren(
                self.to_string(),
                format!("{:?}", object),
            ));
        }
        if let Some(FieldValue::Objects(children)) = last_val {
            children.push(new_child);
        } else {
            return Err(ValuePathError::NotChildren(
                self.to_string(),
                format!("{:?}", object),
            ));
        }
        Ok(())
    }

    pub fn set_in_object(&self, object: &mut Object, value: FieldValue) {
        let mut i_path = self.path.iter().map(|v| match v {
            ValuePathComponent::Index(i) => ValuePathComponent::Index(*i),
            ValuePathComponent::Key(k) => ValuePathComponent::Key(k.to_owned()),
        });
        let mut last_val = None;
        while let Some(cmp) = i_path.next() {
            if last_val.is_none() {
                // At the root, we must have a key string
                if let ValuePathComponent::Key(k) = cmp {
                    if i_path.len() > 0 {
                        last_val = object.values.get_mut(&k);
                        continue;
                    } else {
                        object.values.insert(k, value);
                        break;
                    }
                }
            } else {
                // more than one level deep. We only allow accessing child
                // values, not children themselves - so this finds a child at
                // the index and then finds a key on it.
                if let Some(FieldValue::Objects(children)) = last_val {
                    if let ValuePathComponent::Index(index) = cmp {
                        if let Some(child) = children.get_mut(index) {
                            if let Some(ValuePathComponent::Key(k)) = i_path.next() {
                                if i_path.len() > 0 {
                                    last_val = child.get_mut(&k);
                                    continue;
                                } else {
                                    child.insert(k, value);
                                }
                            }
                        }
                    }
                }
            }
            break;
        }
    }
}
