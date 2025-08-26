use crate::{
    fields::{meta::Meta, FieldType, FieldValue, MetaValue, ObjectValues},
    object::Object,
    ObjectDefinition,
};
use liquid::ValueView;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Display};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValuePathError {
    #[error("Child definition not found for path {0} in {1}")]
    ChildDefNotFound(ValuePath, String),
    #[error("Path {0} was not a children type in {1}")]
    NotChildren(ValuePath, String),
    #[error("Path {0} was not found in {1}")]
    NotFound(ValuePath, String),
    #[error("Child path was missing an index {0}")]
    ChildPathMissingIndex(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ValuePathComponent {
    Key(String),
    Index(usize),
}

impl From<&String> for ValuePathComponent {
    fn from(value: &String) -> Self {
        ValuePath::key(value)
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

#[derive(Debug, Default, Clone, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
#[cfg_attr(feature = "typescript", serde(transparent))]
pub struct ValuePath(
    #[cfg_attr(feature = "typescript", type_def(type_of = "String"))] Vec<ValuePathComponent>,
);

impl ValuePath {
    pub fn key(name: &str) -> ValuePathComponent {
        if name.contains(".") {
            panic!("ValuePathComponent Keys may not contain a dot")
        }
        ValuePathComponent::Key(name.to_string())
    }
    pub fn index(index: usize) -> ValuePathComponent {
        ValuePathComponent::Index(index)
    }
}

impl Display for ValuePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .map(|p| format!("{}", p))
                .collect::<Vec<String>>()
                .join(".")
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
        Self(iter.into_iter().collect())
    }
}

impl ValuePath {
    pub fn empty() -> Self {
        Self(vec![])
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
        Self(vpv)
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn child_name(&self) -> Option<&str> {
        let len = self.len();
        if len < 2 {
            None
        } else {
            let last_components = &self.0.as_slice()[len - 2..];
            if let ValuePathComponent::Key(child_name) = &last_components[0] {
                if matches!(last_components[1], ValuePathComponent::Index(_)) {
                    Some(child_name)
                } else {
                    None
                }
            } else {
                None
            }
        }
    }

    pub fn append(mut self, component: ValuePathComponent) -> Self {
        self.0.push(component);
        self
    }
    pub fn concat(mut self, path: ValuePath) -> Self {
        for p in path.0 {
            self = self.append(p);
        }
        self
    }

    pub fn first(&self) -> ValuePath {
        let first = self.0.first().expect("called .first on an empty ValuePath");
        ValuePath(vec![first.clone()])
    }

    pub fn unshift(&mut self) -> Option<ValuePathComponent> {
        if !self.0.is_empty() {
            Some(self.0.remove(0))
        } else {
            None
        }
    }

    pub fn without_first(&self) -> ValuePath {
        ValuePath(self.0[1..].to_vec())
    }
    pub fn without_last(&self) -> ValuePath {
        ValuePath(self.0[..1].to_vec())
    }

    pub fn pop(&mut self) -> Option<ValuePathComponent> {
        self.0.pop()
    }

    pub fn get_in_meta<'a>(&self, meta: &'a Meta) -> Option<&'a MetaValue> {
        let mut i_path = self.0.iter().map(|v| match v {
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
        let mut i_path = self.0.iter().map(|v| match v {
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
                    return Err(ValuePathError::NotFound(self.clone(), field.to_string()));
                }
                ValuePathComponent::Key(k) => match last_val {
                    FieldValue::Meta(m) => {
                        let c = cmp.clone();
                        return ValuePath::from_iter(vec![c].into_iter().chain(i_path))
                            .get_in_meta(m)
                            .map(FoundValue::Meta)
                            .ok_or_else(|| {
                                ValuePathError::NotFound(self.clone(), field.to_string())
                            });
                    }
                    FieldValue::File(f) => {
                        return f.get_key(k).map(FoundValue::String).ok_or_else(|| {
                            ValuePathError::NotFound(self.clone(), field.to_string())
                        })
                    }
                    _ => return Err(ValuePathError::NotFound(self.clone(), field.to_string())),
                },
            }
        }
        Err(ValuePathError::NotFound(self.clone(), field.to_string()))
    }

    pub fn get_in_object<'a>(&self, object: &'a Object) -> Option<&'a FieldValue> {
        let mut i_path = self.0.iter().map(|v| match v {
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
        let mut i_path = self.0.iter().map(|v| match v {
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
        let mut i_path = self.0.iter().map(|v| match v {
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
        for cmp in self.0.iter() {
            match cmp {
                ValuePathComponent::Key(k) => {
                    if let Some(field) = current_def.fields.get(k) {
                        return Ok(field);
                    } else if let Some(child) = current_def.children.get(k) {
                        current_def = child;
                        continue;
                    } else {
                        return Err(ValuePathError::NotFound(
                            self.clone(),
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
            self.clone(),
            format!("{:?}", &def),
        ))
    }

    pub fn get_definition<'a>(
        &self,
        def: &'a ObjectDefinition,
    ) -> Result<&'a ObjectDefinition, ValuePathError> {
        let mut last_val = def;
        for cmp in self.0.iter() {
            match cmp {
                ValuePathComponent::Key(k) => {
                    if let Some(child_def) = last_val.children.get(k) {
                        last_val = child_def;
                        continue;
                    }
                }
                ValuePathComponent::Index(_) => {
                    // Skip indexes in definitions
                    continue;
                }
            }
            return Err(ValuePathError::ChildDefNotFound(
                self.clone(),
                format!("{:?}", def),
            ));
        }
        Ok(last_val)
    }

    pub fn add_child(
        &self,
        object: &mut Object,
        index: Option<usize>,
        modify: impl FnOnce(&mut BTreeMap<String, FieldValue>) -> Result<(), ValuePathError>,
    ) -> Result<usize, ValuePathError> {
        let mut new_child = BTreeMap::new();
        modify(&mut new_child)?;
        self.modify_children(object, |children| {
            if let Some(index) = index {
                children.insert(index, new_child);
                index
            } else {
                children.push(new_child);
                children.len() - 1
            }
        })
    }

    pub fn remove_child(&mut self, object: &mut Object) -> Result<(), ValuePathError> {
        if let Some(component) = self.pop() {
            match component {
                ValuePathComponent::Index(index) => self.modify_children(object, |children| {
                    children.remove(index);
                }),
                ValuePathComponent::Key(_) => Err(ValuePathError::ChildPathMissingIndex(
                    self.clone().append(component).to_string(),
                )),
            }
        } else {
            Err(ValuePathError::ChildPathMissingIndex(self.to_string()))
        }
    }
    pub fn modify_children<R>(
        &self,
        object: &mut Object,
        modify: impl FnOnce(&mut Vec<ObjectValues>) -> R,
    ) -> Result<R, ValuePathError> {
        let mut i_path = self.0.iter().map(|v| match v {
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
                                if child.contains_key(&k) {
                                    last_val = child.get_mut(&k);
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
            return Err(ValuePathError::NotChildren(
                self.clone(),
                format!("{:?}", object),
            ));
        }
        if let Some(FieldValue::Objects(children)) = last_val {
            Ok(modify(children))
        } else {
            Err(ValuePathError::NotChildren(
                self.clone(),
                format!("{:?}", object),
            ))
        }
    }

    pub fn set_in_tree(
        &self,
        child: &mut BTreeMap<String, FieldValue>,
        value: Option<FieldValue>,
    ) -> Result<(), ValuePathError> {
        let mut path = self.0.clone();
        if let ValuePathComponent::Key(key) = path.remove(0) {
            if self.0.len() == 1 {
                // On the last node, either remove or set the value
                if let Some(value) = value {
                    child.insert(key, value);
                } else {
                    child.remove(&key);
                }
                return Ok(());
            } else if let Some(FieldValue::Objects(children)) = child.get_mut(&key) {
                if let ValuePathComponent::Index(idx) = path.remove(0) {
                    if let Some(child) = children.get_mut(idx) {
                        return ValuePath::from(path).set_in_tree(child, value);
                    }
                }
            }
        }
        Err(ValuePathError::NotFound(
            self.clone(),
            format!("{:?}", child),
        ))
    }

    pub fn set_in_object(
        &self,
        object: &mut Object,
        value: Option<FieldValue>,
    ) -> Result<(), ValuePathError> {
        let mut i_path = self.0.iter().map(|v| match v {
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
                        while children.len() <= index {
                            // No child yet - since we're setting, insert one
                            // here.
                            children.push(ObjectValues::new());
                        }
                        let child = children.get_mut(index).unwrap();
                        ValuePath::from(i_path.collect::<Vec<ValuePathComponent>>())
                            .set_in_tree(child, value)?;
                    }
                }
            }
            break;
        }
        Ok(())
    }
}

impl From<&str> for ValuePath {
    fn from(value: &str) -> Self {
        Self::from_string(value)
    }
}

impl From<Vec<ValuePathComponent>> for ValuePath {
    fn from(value: Vec<ValuePathComponent>) -> Self {
        Self(value)
    }
}

impl serde::Serialize for ValuePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_str())
    }
}
impl<'de> Deserialize<'de> for ValuePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        Ok(Self::from_string(&s))
    }
}

#[cfg(test)]
pub mod tests {

    use super::*;
    use std::error::Error;

    fn object() -> Object {
        Object {
            filename: "test_filename".to_string(),
            object_name: "test_object_name".to_string(),
            order: None,
            path: "".to_string(),
            values: ObjectValues::from([
                ("title".to_string(), FieldValue::String("title".to_string())),
                ("children".to_string(), tree().remove("children").unwrap()),
            ]),
        }
    }

    fn tree() -> BTreeMap<String, FieldValue> {
        let mut tree = BTreeMap::new();
        tree.insert("foo".to_string(), FieldValue::String("bar".to_string()));
        tree.insert(
            "children".to_string(),
            FieldValue::Objects(vec![
                ObjectValues::from([(
                    "name".to_string(),
                    FieldValue::String("NAME ONE!".to_string()),
                )]),
                ObjectValues::from([(
                    "name".to_string(),
                    FieldValue::String("NAME TWO!".to_string()),
                )]),
            ]),
        );
        tree
    }

    #[test]
    fn get_object_values() -> Result<(), Box<dyn Error>> {
        let object = object();
        let child_vp = ValuePath::from_string("children.1");

        let children = child_vp.get_object_values(&object);
        println!("{:?}", children);
        assert!(children.is_some());

        Ok(())
    }

    #[test]
    fn set_in_tree_with_new_value() {
        let mut tree = tree();

        let vp = ValuePath::from_string("children.1.name");
        let new_val = FieldValue::String("NEW NAME".to_string());
        vp.set_in_tree(&mut tree, Some(new_val.clone())).unwrap();

        let children = tree.get("children").unwrap();
        assert!(matches!(children, FieldValue::Objects(_)));
        if let FieldValue::Objects(o) = children {
            let val = o[1].get("name");
            assert_eq!(val.unwrap(), &new_val);
        }
    }

    #[test]
    fn set_in_tree_with_none() {
        let mut tree = tree();

        let vp = ValuePath::from_string("children.1.name");
        vp.set_in_tree(&mut tree, None).unwrap();
        let children = tree.get("children").unwrap();
        assert!(matches!(children, FieldValue::Objects(_)));
        if let FieldValue::Objects(o) = children {
            assert!(!o[1].contains_key("name"));
        }
    }

    #[test]
    fn add_child() {
        let mut obj = object();

        let new_val = FieldValue::String("NEW NAME".to_string());
        let vp = ValuePath::from_string("children");
        let children = obj.values.get("children").unwrap();
        assert!(matches!(children, FieldValue::Objects(_)));
        let prev_len = if let FieldValue::Objects(o) = children {
            o.len()
        } else {
            panic!("not objects");
        };
        vp.add_child(&mut obj, None, |existing| {
            ValuePath::from_string("name").set_in_tree(existing, Some(new_val.clone()))
        })
        .unwrap();

        let children = obj.values.get("children").unwrap();
        assert!(matches!(children, FieldValue::Objects(_)));
        if let FieldValue::Objects(o) = children {
            assert_eq!(o.len(), prev_len + 1);
            let val = o.last().unwrap().get("name");
            assert_eq!(val.unwrap(), &new_val);
        }
    }

    #[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
    struct VPHolder {
        value_path: ValuePath,
    }

    #[test]
    fn serialize_deserialize() {
        let value_path = ValuePath::from_string("children.1.name.0.field");
        let holder = VPHolder { value_path };
        let json_string = serde_json::to_string(&holder).expect("serialization failed.");
        assert_eq!(json_string, r#"{"value_path":"children.1.name.0.field"}"#);
        let deserialized: VPHolder =
            serde_json::from_str(&json_string).expect("deserialization failed.");
        assert_eq!(holder, deserialized);
    }
}
