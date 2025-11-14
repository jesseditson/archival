use crate::{
    object::{Object, Renderable, RenderedObject},
    FieldConfig,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub enum RenderedObjectEntry {
    List(Vec<RenderedObject>),
    Object(RenderedObject),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Hash)]
pub enum ObjectEntry {
    List(Vec<Object>),
    Object(Object),
}

impl Renderable for ObjectEntry {
    type Output = RenderedObjectEntry;
    fn rendered(self, field_config: &FieldConfig) -> Self::Output {
        match self {
            Self::List(list) => RenderedObjectEntry::List(
                list.into_iter()
                    .map(|li| li.rendered(field_config))
                    .collect(),
            ),
            Self::Object(o) => RenderedObjectEntry::Object(o.rendered(field_config)),
        }
    }
}

impl ObjectEntry {
    pub fn empty_list() -> Self {
        Self::List(vec![])
    }
    pub fn from_vec(vec: Vec<Object>) -> Self {
        Self::List(vec)
    }
    pub fn from_object(object: Object) -> Self {
        Self::Object(object)
    }
    pub fn is_list(&self) -> bool {
        matches!(self, ObjectEntry::List(_))
    }
    pub fn is_object(&self) -> bool {
        matches!(self, ObjectEntry::Object(_))
    }
    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Self::Object(o) => Some(o),
            _ => None,
        }
    }
    pub fn as_list(&self) -> Option<&Vec<Object>> {
        match self {
            Self::List(l) => Some(l),
            _ => None,
        }
    }
}

pub struct ObjectEntryIterator<'a> {
    entry: &'a ObjectEntry,
    index: usize,
}

impl<'a> Iterator for ObjectEntryIterator<'a> {
    type Item = &'a Object;
    fn next(&mut self) -> Option<Self::Item> {
        match self.entry {
            ObjectEntry::List(l) => {
                let o = l.get(self.index);
                self.index += 1;
                o
            }
            ObjectEntry::Object(o) => {
                if self.index == 0 {
                    self.index = usize::MAX;
                    Some(o)
                } else {
                    None
                }
            }
        }
    }
}

impl<'a> ObjectEntry {
    pub fn iter_mut(&'a mut self) -> ObjectEntryMutIterator<'a> {
        self.into_iter()
    }
}

impl<'a> IntoIterator for &'a ObjectEntry {
    type Item = &'a Object;
    type IntoIter = ObjectEntryIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ObjectEntryIterator {
            entry: self,
            index: 0,
        }
    }
}

impl<'a> IntoIterator for &'a mut ObjectEntry {
    type Item = &'a mut Object;
    type IntoIter = ObjectEntryMutIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        ObjectEntryMutIterator {
            entry: self,
            index: 0,
        }
    }
}
pub struct ObjectEntryMutIterator<'a> {
    entry: &'a mut ObjectEntry,
    index: usize,
}

impl<'a> Iterator for ObjectEntryMutIterator<'a> {
    type Item = &'a mut Object;
    fn next(&mut self) -> Option<Self::Item> {
        match self.entry {
            ObjectEntry::List(l) => {
                let o = l.get_mut(self.index);
                self.index += 1;
                unsafe { std::mem::transmute::<Option<&mut Object>, Option<&mut Object>>(o) }
            }
            ObjectEntry::Object(o) => {
                if self.index == 0 {
                    self.index = usize::MAX;
                    unsafe { std::mem::transmute::<&mut Object, Option<&mut Object>>(o) }
                } else {
                    None
                }
            }
        }
    }
}
