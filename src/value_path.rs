use crate::{
    fields::{field_value, meta::Meta, FieldType, FieldValue, MetaValue, ObjectValues},
    object::Object,
    ObjectDefinition,
};
use liquid::ValueView;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValuePathError {
    #[error("Child definition not found for path {0} in {1}")]
    ChildDefNotFound(String, String),
    #[error("Path {0} was not a children type in {1}")]
    NotChildren(String, String),
    #[error("Path {0} was not found in {1}")]
    NotFound(String, String),
    #[error("Cannot remove {0}")]
    InvalidRemovePath(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub enum ValuePathComponent {
    Key(String),
    Index(usize),
}

impl ValuePathComponent {
    pub fn key(name: &str) -> Self {
        Self::Key(name.to_string())
    }
    pub fn as_key(vp: Option<Self>) -> Option<String> {
        match vp {
            Some(vp) => match vp {
                ValuePathComponent::Key(k) => Some(k),
                ValuePathComponent::Index(_) => None,
            },
            None => None,
        }
    }
    pub fn as_index(vp: Option<Self>) -> Option<usize> {
        match vp {
            Some(vp) => match vp {
                ValuePathComponent::Key(_) => None,
                ValuePathComponent::Index(i) => Some(i),
            },
            None => None,
        }
    }
}

impl From<&String> for ValuePathComponent {
    fn from(value: &String) -> Self {
        ValuePathComponent::Key(value.to_string())
    }
}
impl From<usize> for ValuePathComponent {
    fn from(value: usize) -> Self {
        ValuePathComponent::Index(value)
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

#[derive(Debug, Default, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub struct ValuePath {
    path: Vec<ValuePathComponent>,
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

pub enum FoundValue<'a> {
    Meta(&'a MetaValue),
    String(&'a String),
}

impl Display for FoundValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(s) => write!(f, "{}", s),
            Self::Meta(mv) => write!(f, "{}", mv.render()),
        }
    }
}

impl FromIterator<ValuePathComponent> for ValuePath {
    fn from_iter<T: IntoIterator<Item = ValuePathComponent>>(iter: T) -> Self {
        Self {
            path: iter.into_iter().collect(),
        }
    }
}

impl ValuePath {
    pub fn empty() -> Self {
        Self { path: vec![] }
    }
    pub fn from_string(string: &str) -> Self {
        let mut vpv: Vec<ValuePathComponent> = vec![];
        if !string.is_empty() {
            for part in string.split('.') {
                match part.parse::<usize>() {
                    Ok(index) => vpv.push(ValuePathComponent::Index(index)),
                    Err(_) => vpv.push(ValuePathComponent::Key(part.to_string())),
                }
            }
        }
        Self { path: vpv }
    }
    pub fn is_empty(&self) -> bool {
        self.path.is_empty()
    }

    pub fn append(mut self, component: ValuePathComponent) -> Self {
        self.path.push(component);
        self
    }
    pub fn concat(mut self, path: ValuePath) -> Self {
        for p in path.path {
            self = self.append(p);
        }
        self
    }

    pub fn first(&self) -> ValuePath {
        let first = self
            .path
            .first()
            .expect("called .first on an empty value_path");
        ValuePath {
            path: vec![first.clone()],
        }
    }

    pub fn unshift(&mut self) -> Option<ValuePathComponent> {
        if !self.path.is_empty() {
            Some(self.path.remove(0))
        } else {
            None
        }
    }

    pub fn pop(&mut self) -> Option<ValuePathComponent> {
        self.path.pop()
    }

    pub fn get_in_meta<'a>(&self, meta: &'a Meta) -> Option<&'a MetaValue> {
        let mut i_path = self.path.iter().map(|v| match v {
            ValuePathComponent::Index(i) => ValuePathComponent::Index(*i),
            ValuePathComponent::Key(k) => ValuePathComponent::Key(k.to_owned()),
        });
        let path = if let Some(ValuePathComponent::Key(k)) = i_path.next() {
            k
        } else {
            return None;
        };
        let mut last_val = meta.get_value(&path)?;
        for cmp in i_path {
            match cmp {
                ValuePathComponent::Index(i) => {
                    if let MetaValue::Array(a) = last_val {
                        if let Some(f) = a.get(i) {
                            last_val = f;
                            continue;
                        }
                    }
                    return None;
                }
                ValuePathComponent::Key(k) => match last_val {
                    MetaValue::Map(m) => {
                        if let Some(f) = m.get_value(&k) {
                            last_val = f;
                            continue;
                        }
                    }
                    _ => {
                        return None;
                    }
                },
            }
        }
        Some(last_val)
    }

    pub fn get_value<'a>(&self, field: &'a FieldValue) -> Result<FoundValue<'a>, ValuePathError> {
        let mut i_path = self.path.iter().map(|v| match v {
            ValuePathComponent::Index(i) => ValuePathComponent::Index(*i),
            ValuePathComponent::Key(k) => ValuePathComponent::Key(k.to_owned()),
        });
        let mut last_val = field;
        while let Some(cmp) = &i_path.next() {
            match cmp {
                ValuePathComponent::Index(i) => {
                    if let FieldValue::Objects(o) = field {
                        if let Some(v) = o.get(*i) {
                            if let Some(ValuePathComponent::Key(k)) = i_path.next() {
                                if let Some(fv) = v.get(&k) {
                                    last_val = fv;
                                    continue;
                                }
                            }
                        }
                    }
                    return Err(ValuePathError::NotFound(
                        self.to_string(),
                        field.to_string(),
                    ));
                }
                ValuePathComponent::Key(k) => match last_val {
                    FieldValue::Meta(m) => {
                        let c = cmp.clone();
                        return ValuePath::from_iter(vec![c].into_iter().chain(i_path))
                            .get_in_meta(m)
                            .map(FoundValue::Meta)
                            .ok_or_else(|| {
                                ValuePathError::NotFound(self.to_string(), field.to_string())
                            });
                    }
                    FieldValue::File(f) => {
                        return f.get_key(k).map(FoundValue::String).ok_or_else(|| {
                            ValuePathError::NotFound(self.to_string(), field.to_string())
                        })
                    }
                    _ => {
                        return Err(ValuePathError::NotFound(
                            self.to_string(),
                            field.to_string(),
                        ))
                    }
                },
            }
        }
        Err(ValuePathError::NotFound(
            self.to_string(),
            field.to_string(),
        ))
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

    pub fn get_children<'a>(&self, object: &'a Object) -> Option<&'a Vec<ObjectValues>> {
        let mut i_path = self.path.iter().map(|v| match v {
            ValuePathComponent::Index(i) => ValuePathComponent::Index(*i),
            ValuePathComponent::Key(k) => ValuePathComponent::Key(k.to_owned()),
        });
        let mut last_val = &object.values;
        while let Some(cmp) = i_path.next() {
            if let ValuePathComponent::Key(key) = cmp {
                if let Some(FieldValue::Objects(children)) = last_val.get(&key) {
                    let next = i_path.next();
                    if next.is_none() {
                        // Reached the end of the path, return the
                        // children here.
                        return Some(children);
                    } else if let Some(ValuePathComponent::Index(k)) = next {
                        // Path continues, recurse.
                        if let Some(c) = children.get(k) {
                            last_val = c;
                            continue;
                        }
                    } else {
                        // Path continues but next item is not an index,
                        // so this is not a valid path. Return nothing.
                        return None;
                    }
                }
            }
            break;
        }
        None
    }

    pub fn get_object_values<'a>(&self, object: &'a Object) -> Option<&'a ObjectValues> {
        let mut i_path = self.path.iter().map(|v| match v {
            ValuePathComponent::Index(i) => ValuePathComponent::Index(*i),
            ValuePathComponent::Key(k) => ValuePathComponent::Key(k.to_owned()),
        });
        let mut last_val = &object.values;
        while let Some(cmp) = i_path.next() {
            if let ValuePathComponent::Key(k) = cmp {
                if let Some(FieldValue::Objects(children)) = last_val.get(&k) {
                    if let Some(ValuePathComponent::Index(idx)) = i_path.next() {
                        if let Some(child) = children.get(idx) {
                            last_val = child;
                        }
                    } else {
                        panic!("invalid value path {} for {:?}", self, object);
                    }
                } else {
                    return None;
                }
            } else {
                panic!("invalid value path {} for {:?}", self, object);
            }
        }
        Some(last_val)
    }

    pub fn get_field_definition<'a>(
        &self,
        def: &'a ObjectDefinition,
    ) -> Result<&'a FieldType, ValuePathError> {
        let mut current_def = def;
        for cmp in self.path.iter() {
            match cmp {
                ValuePathComponent::Key(k) => {
                    if let Some(field) = current_def.fields.get(k) {
                        return Ok(field);
                    } else if let Some(child) = current_def.children.get(k) {
                        current_def = child;
                        continue;
                    } else {
                        return Err(ValuePathError::NotFound(
                            self.to_string(),
                            format!("{:?}", &def),
                        ));
                    }
                }
                ValuePathComponent::Index(_) => {
                    // Value Paths point to specific children, so when looking
                    // them up in definitions, we just skip over indexes.
                    continue;
                }
            }
        }
        Err(ValuePathError::NotFound(
            self.to_string(),
            format!("{:?}", &def),
        ))
    }

    pub fn get_definition<'a>(
        &self,
        def: &'a ObjectDefinition,
    ) -> Result<&'a ObjectDefinition, ValuePathError> {
        let mut last_val = def;
        for cmp in self.path.iter() {
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
        Ok(last_val)
    }

    pub fn add_child(
        &self,
        object: &mut Object,
        obj_def: &ObjectDefinition,
    ) -> Result<usize, ValuePathError> {
        let child_def = self.get_definition(obj_def)?;
        let new_child = field_value::def_to_values(&child_def.fields);
        self.modify_children(object, |children| {
            children.push(new_child);
            children.len() - 1
        })
    }

    pub fn remove_child(&mut self, object: &mut Object) -> Result<(), ValuePathError> {
        if let Some(ValuePathComponent::Index(index)) = self.pop() {
            self.modify_children(object, |children| {
                children.remove(index);
            })
        } else {
            Err(ValuePathError::InvalidRemovePath(self.to_string()))
        }
    }
    fn modify_children<R>(
        &self,
        object: &mut Object,
        modify: impl FnOnce(&mut Vec<ObjectValues>) -> R,
    ) -> Result<R, ValuePathError> {
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
            Ok(modify(children))
        } else {
            Err(ValuePathError::NotChildren(
                self.to_string(),
                format!("{:?}", object),
            ))
        }
    }

    pub fn set_in_object(&self, object: &mut Object, value: Option<FieldValue>) {
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
                        match value {
                            Some(value) => object.values.insert(k, value),
                            None => object.values.remove(&k),
                        };
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
                                    match value {
                                        Some(value) => child.insert(k, value),
                                        None => child.remove(&k),
                                    };
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

impl From<&str> for ValuePath {
    fn from(value: &str) -> Self {
        Self::from_string(value)
    }
}
