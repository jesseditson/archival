mod generated {
    #![allow(clippy::all, clippy::pedantic, clippy::nursery)]
    pub mod archival_capnp;
    pub mod util_capnp;
}

// use crate::{
//     events::{
//         AddChildEvent, AddObjectEvent, AddObjectValue, AddRootObjectEvent, ArchivalEvent,
//         DeleteObjectEvent, EditFieldEvent, EditOrderEvent, RemoveChildEvent, RenameObjectEvent,
//     },
//     fields::{
//         meta::{Meta, MetaMap},
//         DisplayType, FieldType, File, MetaValue, ObjectValues,
//     },
//     object::{Object, ObjectEntry, ValuePath, ValuePathComponent},
//     object_definition::FieldsMap,
//     FieldValue, ObjectDefinition, ObjectMap,
// };

pub use generated::archival_capnp;
pub use generated::util_capnp;

// impl<'a, T> util_capnp::option::Reader<'a, T>
// where
//     T: capnp::traits::Owned,
// {
//     pub fn and_then<U, F>(self, f: F) -> Option<U>
//     where
//         F: FnOnce(<T as capnp::traits::Owned>::Reader<'_>) -> Option<U>,
//     {
//         self.which()
//             .map(|v| match v {
//                 util_capnp::option::Which::None(_) => None,
//                 util_capnp::option::Which::Some(v) => f(v.unwrap()),
//             })
//             .unwrap_or_default()
//     }
// }

// impl<'a> From<archival_capnp::display_type::Reader<'a>> for DisplayType {
//     fn from(value: archival_capnp::display_type::Reader<'a>) -> Self {
//         value
//             .which()
//             .map(|val| match val {
//                 archival_capnp::display_type::Which::Image(_) => Self::Image,
//                 archival_capnp::display_type::Which::Video(_) => Self::Video,
//                 archival_capnp::display_type::Which::Audio(_) => Self::Audio,
//                 archival_capnp::display_type::Which::Download(_) => Self::Download,
//             })
//             .unwrap_or_default()
//     }
// }

// impl<'a> From<archival_capnp::meta_value::Reader<'a>> for MetaValue {
//     fn from(value: archival_capnp::meta_value::Reader<'a>) -> Self {
//         match value.which() {
//             Ok(archival_capnp::meta_value::Which::String(s)) => {
//                 MetaValue::String(s.ok().and_then(|s| s.to_string().ok()).unwrap_or_default())
//             }
//             Ok(archival_capnp::meta_value::Which::Number(n)) => MetaValue::Number(n),
//             Ok(archival_capnp::meta_value::Which::Boolean(b)) => MetaValue::Boolean(b),
//             Ok(archival_capnp::meta_value::Which::DateString(s)) => {
//                 let date_str = s.ok().and_then(|s| s.to_str().ok()).unwrap_or_default();
//                 crate::fields::DateTime::from(date_str)
//                     .map(MetaValue::DateTime)
//                     .unwrap_or(MetaValue::String(date_str.to_string()))
//             }
//             Ok(archival_capnp::meta_value::Which::Array(arr)) => {
//                 MetaValue::Array(arr.unwrap().iter().map(|mv| MetaValue::from(mv)).collect())
//             }
//             Ok(archival_capnp::meta_value::Which::Map(map_list)) => {
//                 MetaValue::Map(Meta::from(map_list.unwrap()))
//             }
//             Err(_) => MetaValue::String(String::new()),
//         }
//     }
// }

// impl<'a> From<archival_capnp::meta::Reader<'a>> for Meta {
//     fn from(value: archival_capnp::meta::Reader<'a>) -> Self {
//         let mut meta = MetaMap::new();
//         if let Ok(entries) = value.get_entries() {
//             for entry in entries {
//                 if let Ok(key) = entry.get_name() {
//                     if let Ok(val) = entry.get_value() {
//                         meta.insert(key.to_string().unwrap(), MetaValue::from(val));
//                     }
//                 }
//             }
//         }
//         Meta(meta)
//     }
// }

// impl<'a> From<archival_capnp::meta::entry::Reader<'a>> for (String, MetaValue) {
//     fn from(value: archival_capnp::meta::entry::Reader<'a>) -> Self {
//         let key = value.get_name().unwrap().to_string().unwrap();
//         let val = value
//             .get_value()
//             .map(MetaValue::from)
//             .unwrap_or(MetaValue::String(String::new()));
//         (key, val)
//     }
// }

// impl<'a> From<archival_capnp::file::Reader<'a>> for File {
//     fn from(value: archival_capnp::file::Reader<'a>) -> Self {
//         File {
//             display_type: value
//                 .get_display_type()
//                 .map(|dt| DisplayType::from(dt))
//                 .unwrap_or_default(),
//             filename: value
//                 .get_filename()
//                 .ok()
//                 .and_then(|s| s.to_string().ok())
//                 .unwrap_or_default(),
//             sha: value
//                 .get_sha()
//                 .ok()
//                 .and_then(|s| s.to_string().ok())
//                 .unwrap_or_default(),
//             mime: value
//                 .get_mime()
//                 .ok()
//                 .and_then(|s| s.to_string().ok())
//                 .unwrap_or_default(),
//             name: value
//                 .get_name()
//                 .ok()
//                 .and_then(|r| r.and_then(|v| v.to_string().ok())),
//             description: value
//                 .get_description()
//                 .ok()
//                 .and_then(|r| r.and_then(|v| v.to_string().ok())),
//         }
//     }
// }

// impl<'a> From<archival_capnp::field_value::Reader<'a>> for FieldValue {
//     fn from(value: archival_capnp::field_value::Reader<'a>) -> Self {
//         match value.which() {
//             Ok(archival_capnp::field_value::Which::String(s)) => {
//                 FieldValue::String(s.unwrap().to_string().unwrap())
//             }
//             Ok(archival_capnp::field_value::Which::Markdown(s)) => {
//                 FieldValue::Markdown(s.unwrap().to_string().unwrap())
//             }
//             Ok(archival_capnp::field_value::Which::Number(n)) => FieldValue::Number(n),
//             Ok(archival_capnp::field_value::Which::Boolean(b)) => FieldValue::Boolean(b),
//             Ok(archival_capnp::field_value::Which::DateString(dt)) => {
//                 let dt_str = dt.unwrap().to_str().unwrap();
//                 FieldValue::Date(crate::fields::DateTime::from(dt_str).expect("invalid date"))
//             }
//             Ok(archival_capnp::field_value::Which::File(f)) => {
//                 FieldValue::File(File::from(f.unwrap()))
//             }
//             // Ok(archival_capnp::field_value::Which::Objects(o)) => {
//             //     FieldValue::Objects()
//             // }
//             // Ok(archival_capnp::field_value::Which::Meta(m)) => {
//             //     FieldValue::Meta(Meta::from(m.unwrap()))
//             // }
//             Err(_) | _ => unreachable!("invalid field"),
//         }
//     }
// }

// impl<'a> From<archival_capnp::object_values::Reader<'a>> for ObjectValues {
//     fn from(value: archival_capnp::object_values::Reader<'a>) -> Self {
//         let mut values = ObjectValues::new();
//         if let Ok(fields) = value.get_fields() {
//             for field in fields {
//                 if let Ok(key) = field.get_name() {
//                     if let Ok(val) = field.get_value() {
//                         values.insert(key.to_string().unwrap(), FieldValue::from(val));
//                     }
//                 }
//             }
//         }
//         values
//     }
// }

// impl<'a> From<archival_capnp::object::Reader<'a>> for Object {
//     fn from(value: archival_capnp::object::Reader<'a>) -> Self {
//         let object_name = value
//             .get_object_name()
//             .ok()
//             .and_then(|r| r.to_string().ok())
//             .unwrap_or_default();
//         let filename = value
//             .get_filename()
//             .ok()
//             .and_then(|r| r.to_string().ok())
//             .unwrap_or_default();
//         let path = value
//             .get_path()
//             .ok()
//             .and_then(|r| r.to_string().ok())
//             .unwrap_or_default();
//         let order = value
//             .get_order()
//             .which()
//             .map(|v| match v {
//                 archival_capnp::object::order::Which::None(_) => None,
//                 archival_capnp::object::order::Which::Some(v) => Some(v),
//             })
//             .unwrap_or_default();

//         let values = value
//             .get_values()
//             .ok()
//             .and_then(|ov| Some(ObjectValues::from(ov)))
//             .unwrap_or_default();

//         Object {
//             object_name,
//             filename,
//             path,
//             order,
//             values,
//         }
//     }
// }

// // impl<'a> From<archival_capnp::object_values::Reader<'a>> for ObjectValues {
// //     fn from(value: archival_capnp::object_values::Reader<'a>) -> Self {
// //         let mut map = ObjectValues::new();
// //         if let Ok(entries) = value.get_fields() {
// //             for entry in entries {
// //                 if let Ok(key) = entry.get_name() {
// //                     if let Ok(val) = entry.get_value() {
// //                         map.insert(key.to_string().unwrap(), FieldValue::from(val));
// //                     }
// //                 }
// //             }
// //         }
// //         map
// //     }
// // }

// impl<'a> From<archival_capnp::object_entry::Reader<'a>> for ObjectEntry {
//     fn from(value: archival_capnp::object_entry::Reader<'a>) -> Self {
//         match value.which() {
//             Ok(archival_capnp::object_entry::Which::List(list)) => {
//                 ObjectEntry::List(list.unwrap().iter().map(|o| Object::from(o)).collect())
//             }
//             Ok(archival_capnp::object_entry::Which::Object(obj)) => {
//                 ObjectEntry::Object(Object::from(obj.unwrap()))
//             }
//             Err(_) => ObjectEntry::List(vec![]),
//         }
//     }
// }

// impl<'a> From<archival_capnp::object_map::Reader<'a>> for ObjectMap {
//     fn from(value: archival_capnp::object_map::Reader<'a>) -> Self {
//         let mut map = ObjectMap::new();
//         if let Ok(entries) = value.get_entries() {
//             for entry in entries {
//                 if let Ok(key) = entry.get_name() {
//                     if let Ok(val) = entry.get_object() {
//                         map.insert(key.to_string().unwrap(), ObjectEntry::from(val));
//                     }
//                 }
//             }
//         }
//         map
//     }
// }

// impl<'a> From<archival_capnp::field_type::Reader<'a>> for FieldType {
//     fn from(value: archival_capnp::field_type::Reader<'a>) -> Self {
//         match value.which() {
//             Ok(archival_capnp::field_type::Which::String(_)) => FieldType::String,
//             Ok(archival_capnp::field_type::Which::Markdown(_)) => FieldType::Markdown,
//             Ok(archival_capnp::field_type::Which::Number(_)) => FieldType::Number,
//             Ok(archival_capnp::field_type::Which::Enum(v)) => {
//                 let enum_vals = v
//                     .ok()
//                     .and_then(|v| {
//                         v.iter()
//                             .map(|i| i.ok().and_then(|typ| typ.to_string().ok()))
//                             .collect()
//                     })
//                     .expect("bad enum");
//                 FieldType::Enum(enum_vals)
//             }
//             Ok(archival_capnp::field_type::Which::Boolean(_)) => FieldType::Boolean,
//             Ok(archival_capnp::field_type::Which::Date(_)) => FieldType::Date,
//             Ok(archival_capnp::field_type::Which::Audio(_)) => FieldType::Audio,
//             Ok(archival_capnp::field_type::Which::Video(_)) => FieldType::Video,
//             Ok(archival_capnp::field_type::Which::Image(_)) => FieldType::Image,
//             Ok(archival_capnp::field_type::Which::Upload(_)) => FieldType::Upload,
//             Ok(archival_capnp::field_type::Which::Meta(m)) => {}
//             Ok(archival_capnp::field_type::Which::Alias(a)) => {
//                 let (field_type, name) = a
//                     .ok()
//                     .and_then(|at| {
//                         if let (Some(name), Some(field_type)) =
//                             (at.get_name().ok(), at.get_type().ok())
//                         {
//                             Some((
//                                 FieldType::from(field_type),
//                                 name.to_string().ok().unwrap_or_default(),
//                             ))
//                         } else {
//                             None
//                         }
//                     })
//                     .unwrap();
//                 FieldType::Alias(Box::new((field_type, name)))
//             }
//             Err(_) => unreachable!("unknown field type"),
//         }
//     }
// }

// impl<'a> From<archival_capnp::fields_map::Reader<'a>> for FieldsMap {
//     fn from(value: archival_capnp::fields_map::Reader<'a>) -> Self {
//         let mut map = FieldsMap::new();
//         if let Ok(fields) = value.get_field() {
//             for field in fields {
//                 if let Ok(key) = field.get_key() {
//                     if let Ok(field_type) = field.get_value() {
//                         map.insert(key.to_string(), FieldType::from(field_type));
//                     }
//                 }
//             }
//         }
//         map
//     }
// }

// impl<'a> From<archival_capnp::object_definition::Reader<'a>> for ObjectDefinition {
//     fn from(value: archival_capnp::object_definition::Reader<'a>) -> Self {
//         let fields = value
//             .get_fields()
//             .ok()
//             .map(FieldsMap::from)
//             .unwrap_or_default();

//         ObjectDefinition {
//             fields,
//             children: Default::default(),
//         }
//     }
// }

// impl<'a> From<archival_capnp::value_path_component::Reader<'a>> for ValuePathComponent {
//     fn from(value: archival_capnp::value_path_component::Reader<'a>) -> Self {
//         match value.which() {
//             Ok(archival_capnp::value_path_component::Which::Key(k)) => {
//                 ValuePathComponent::Key(k.unwrap_or("").to_string())
//             }
//             Ok(archival_capnp::value_path_component::Which::Index(i)) => {
//                 ValuePathComponent::Index(i as usize)
//             }
//             Err(_) => ValuePathComponent::Key(String::new()),
//         }
//     }
// }

// impl<'a> From<archival_capnp::value_path::Reader<'a>> for ValuePath {
//     fn from(value: archival_capnp::value_path::Reader<'a>) -> Self {
//         let components = value
//             .get_components()
//             .unwrap_or_default()
//             .iter()
//             .map(|c| ValuePathComponent::from(c))
//             .collect();
//         ValuePath(components)
//     }
// }

// impl<'a> From<archival_capnp::add_object_value::Reader<'a>> for AddObjectValue {
//     fn from(value: archival_capnp::add_object_value::Reader<'a>) -> Self {
//         AddObjectValue {
//             object: value.get_object().unwrap_or("").to_string(),
//             value: value
//                 .get_value()
//                 .ok()
//                 .map(FieldValue::from)
//                 .unwrap_or(FieldValue::String(String::new())),
//         }
//     }
// }

// impl<'a> From<archival_capnp::rename_object_event::Reader<'a>> for RenameObjectEvent {
//     fn from(value: archival_capnp::rename_object_event::Reader<'a>) -> Self {
//         RenameObjectEvent {
//             object: value.get_object().unwrap_or("").to_string(),
//             new_name: value.get_new_name().unwrap_or("").to_string(),
//         }
//     }
// }

// impl<'a> From<archival_capnp::add_object_event::Reader<'a>> for AddObjectEvent {
//     fn from(value: archival_capnp::add_object_event::Reader<'a>) -> Self {
//         AddObjectEvent {
//             object: value.get_object().unwrap_or("").to_string(),
//             data: value
//                 .get_data()
//                 .ok()
//                 .map(ObjectEntry::from)
//                 .unwrap_or(ObjectEntry::List(vec![])),
//         }
//     }
// }

// impl<'a> From<archival_capnp::add_root_object_event::Reader<'a>> for AddRootObjectEvent {
//     fn from(value: archival_capnp::add_root_object_event::Reader<'a>) -> Self {
//         AddRootObjectEvent {
//             object: value.get_object().unwrap_or("").to_string(),
//             data: value.get_data().ok().map(Object::from).unwrap_or_default(),
//         }
//     }
// }

// impl<'a> From<archival_capnp::delete_object_event::Reader<'a>> for DeleteObjectEvent {
//     fn from(value: archival_capnp::delete_object_event::Reader<'a>) -> Self {
//         DeleteObjectEvent {
//             object: value.get_object().unwrap_or("").to_string(),
//         }
//     }
// }

// impl<'a> From<archival_capnp::edit_field_event::Reader<'a>> for EditFieldEvent {
//     fn from(value: archival_capnp::edit_field_event::Reader<'a>) -> Self {
//         EditFieldEvent {
//             object: value.get_object().unwrap_or("").to_string(),
//             path: value
//                 .get_path()
//                 .ok()
//                 .map(ValuePath::from)
//                 .unwrap_or_default(),
//             value: value
//                 .get_value()
//                 .ok()
//                 .map(FieldValue::from)
//                 .unwrap_or(FieldValue::String(String::new())),
//         }
//     }
// }

// impl<'a> From<archival_capnp::edit_order_event::Reader<'a>> for EditOrderEvent {
//     fn from(value: archival_capnp::edit_order_event::Reader<'a>) -> Self {
//         let order = value
//             .get_order()
//             .ok()
//             .and_then(|o| opt_order_from_reader(o.which().ok()?))
//             .unwrap_or(crate::object::Order::Alphabetical);

//         EditOrderEvent {
//             object: value.get_object().unwrap_or("").to_string(),
//             order,
//         }
//     }
// }

// impl<'a> From<archival_capnp::add_child_event::Reader<'a>> for AddChildEvent {
//     fn from(value: archival_capnp::add_child_event::Reader<'a>) -> Self {
//         AddChildEvent {
//             parent: value.get_parent().unwrap_or("").to_string(),
//             child: value.get_child().unwrap_or("").to_string(),
//             index: value
//                 .get_index()
//                 .ok()
//                 .and_then(|idx| idx.get_index().ok())
//                 .map(|i| i as usize),
//         }
//     }
// }

// impl<'a> From<archival_capnp::remove_child_event::Reader<'a>> for RemoveChildEvent {
//     fn from(value: archival_capnp::remove_child_event::Reader<'a>) -> Self {
//         RemoveChildEvent {
//             parent: value.get_parent().unwrap_or("").to_string(),
//             child: value.get_child().unwrap_or("").to_string(),
//         }
//     }
// }

// impl<'a> From<archival_capnp::archival_event::Reader<'a>> for ArchivalEvent {
//     fn from(value: archival_capnp::archival_event::Reader<'a>) -> Self {
//         match value.which() {
//             Ok(archival_capnp::archival_event::Which::RenameObject(e)) => {
//                 ArchivalEvent::RenameObject(RenameObjectEvent::from(e.unwrap()))
//             }
//             Ok(archival_capnp::archival_event::Which::AddObject(e)) => {
//                 ArchivalEvent::AddObject(AddObjectEvent::from(e.unwrap()))
//             }
//             Ok(archival_capnp::archival_event::Which::AddRootObject(e)) => {
//                 ArchivalEvent::AddRootObject(AddRootObjectEvent::from(e.unwrap()))
//             }
//             Ok(archival_capnp::archival_event::Which::DeleteObject(e)) => {
//                 ArchivalEvent::DeleteObject(DeleteObjectEvent::from(e.unwrap()))
//             }
//             Ok(archival_capnp::archival_event::Which::EditField(e)) => {
//                 ArchivalEvent::EditField(EditFieldEvent::from(e.unwrap()))
//             }
//             Ok(archival_capnp::archival_event::Which::EditOrder(e)) => {
//                 ArchivalEvent::EditOrder(EditOrderEvent::from(e.unwrap()))
//             }
//             Ok(archival_capnp::archival_event::Which::AddChild(e)) => {
//                 ArchivalEvent::AddChild(AddChildEvent::from(e.unwrap()))
//             }
//             Ok(archival_capnp::archival_event::Which::RemoveChild(e)) => {
//                 ArchivalEvent::RemoveChild(RemoveChildEvent::from(e.unwrap()))
//             }
//             Err(_) => unreachable!("not in schema"),
//         }
//     }
// }
