use crate::object::Object;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "typescript", derive(typescript_type_def::TypeDef))]
pub enum ObjectEntry {
    List(Vec<Object>),
    Object(Object),
}

impl ObjectEntry {
    pub fn from_vec(vec: Vec<Object>) -> Self {
        Self::List(vec)
    }
    pub fn from_object(object: Object) -> Self {
        Self::Object(object)
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
                unsafe { std::mem::transmute(o) }
            }
            ObjectEntry::Object(o) => {
                if self.index == 0 {
                    self.index = usize::MAX;
                    unsafe { std::mem::transmute(o) }
                } else {
                    None
                }
            }
        }
    }
}
