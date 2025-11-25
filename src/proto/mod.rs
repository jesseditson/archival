pub mod archival_proto {
    include!(concat!(env!("OUT_DIR"), "/archival_proto.rs"));
}

use crate::fields::meta::Meta;
use crate::fields::{DateTime, DisplayType, FieldType, FieldValue, File, MetaValue};
use crate::object::{Object, ObjectEntry, ObjectMap};
use crate::object_definition::ObjectDefinition;
use crate::value_path::{ValuePath, ValuePathComponent};
use ordermap::OrderMap;
use std::collections::BTreeMap;

// DisplayType
impl From<archival_proto::DisplayType> for DisplayType {
    fn from(proto: archival_proto::DisplayType) -> Self {
        match proto {
            archival_proto::DisplayType::Image => DisplayType::Image,
            archival_proto::DisplayType::Video => DisplayType::Video,
            archival_proto::DisplayType::Audio => DisplayType::Audio,
            archival_proto::DisplayType::Download => DisplayType::Download,
        }
    }
}

// Reverse conversions: from Rust types into proto types

// DisplayType -> proto
impl From<DisplayType> for archival_proto::DisplayType {
    fn from(dt: DisplayType) -> Self {
        match dt {
            DisplayType::Image => archival_proto::DisplayType::Image,
            DisplayType::Video => archival_proto::DisplayType::Video,
            DisplayType::Audio => archival_proto::DisplayType::Audio,
            DisplayType::Download => archival_proto::DisplayType::Download,
        }
    }
}

// File -> proto
impl From<File> for archival_proto::File {
    fn from(f: File) -> Self {
        archival_proto::File {
            display_type: archival_proto::DisplayType::from(f.display_type) as i32,
            filename: f.filename,
            sha: f.sha,
            mime: f.mime,
            name: f.name.unwrap_or_default(),
            description: f.description.unwrap_or_default(),
        }
    }
}

// MetaValue -> proto
impl From<MetaValue> for archival_proto::MetaValue {
    fn from(mv: MetaValue) -> Self {
        let value = match mv {
            MetaValue::String(s) => archival_proto::meta_value::Value::String(s),
            MetaValue::Number(n) => archival_proto::meta_value::Value::Number(n),
            MetaValue::Boolean(b) => archival_proto::meta_value::Value::Boolean(b),
            MetaValue::DateTime(dt) => {
                archival_proto::meta_value::Value::DateString(dt.to_string())
            }
            MetaValue::Array(arr) => {
                archival_proto::meta_value::Value::Array(archival_proto::MetaArray {
                    values: arr.into_iter().map(|v| v.into()).collect(),
                })
            }
            MetaValue::Map(map) => archival_proto::meta_value::Value::Map(map.into()),
        };
        archival_proto::MetaValue { value: Some(value) }
    }
}

// Meta -> proto
impl From<Meta> for archival_proto::Meta {
    fn from(meta: Meta) -> Self {
        let mut entries = Vec::new();
        for (name, value) in meta.0.into_iter() {
            entries.push(archival_proto::meta::Entry {
                name,
                value: Some(value.into()),
            });
        }
        archival_proto::Meta { entries }
    }
}

// FieldValue -> proto
impl From<FieldValue> for archival_proto::FieldValue {
    fn from(fv: FieldValue) -> Self {
        let value = match fv {
            FieldValue::String(s) => archival_proto::field_value::Value::String(s),
            FieldValue::Enum(e) => archival_proto::field_value::Value::Enum(e),
            FieldValue::Markdown(m) => archival_proto::field_value::Value::Markdown(m),
            FieldValue::Number(n) => archival_proto::field_value::Value::Number(n),
            FieldValue::Date(d) => archival_proto::field_value::Value::DateString(d.to_string()),
            FieldValue::Objects(objs) => {
                let mut fields = Vec::new();
                // FieldValue::Objects is Vec<BTreeMap<String, FieldValue>>; proto expects one ObjectValues
                if let Some(first) = objs.into_iter().next() {
                    for (name, val) in first.into_iter() {
                        fields.push(archival_proto::object_values::Field {
                            name,
                            value: Some(val.into()),
                        });
                    }
                }
                archival_proto::field_value::Value::Objects(archival_proto::ObjectValues { fields })
            }
            FieldValue::Boolean(b) => archival_proto::field_value::Value::Boolean(b),
            FieldValue::File(file) => archival_proto::field_value::Value::File(file.into()),
            FieldValue::Meta(m) => archival_proto::field_value::Value::Meta(m.into()),
            FieldValue::Null => archival_proto::field_value::Value::Null(()),
        };
        archival_proto::FieldValue { value: Some(value) }
    }
}

// BTreeMap<String, FieldValue> -> ObjectValues proto
impl From<BTreeMap<String, FieldValue>> for archival_proto::ObjectValues {
    fn from(map: BTreeMap<String, FieldValue>) -> Self {
        let fields = map
            .into_iter()
            .map(|(name, value)| archival_proto::object_values::Field {
                name,
                value: Some(value.into()),
            })
            .collect();
        archival_proto::ObjectValues { fields }
    }
}

// Object -> proto
impl From<Object> for archival_proto::Object {
    fn from(obj: Object) -> Self {
        let order = match obj.order {
            None => Some(archival_proto::object::Order::None(())),
            Some(o) => Some(archival_proto::object::Order::Some(o)),
        };
        archival_proto::Object {
            filename: obj.filename,
            object_name: obj.object_name,
            order,
            path: obj.path,
            values: Some(obj.values.into()),
        }
    }
}

// ObjectEntry -> proto
impl From<ObjectEntry> for archival_proto::ObjectEntry {
    fn from(entry: ObjectEntry) -> Self {
        match entry {
            ObjectEntry::List(list) => archival_proto::ObjectEntry {
                entry: Some(archival_proto::object_entry::Entry::List(
                    archival_proto::ObjectList {
                        entries: list.into_iter().map(|o| o.into()).collect(),
                    },
                )),
            },
            ObjectEntry::Object(obj) => archival_proto::ObjectEntry {
                entry: Some(archival_proto::object_entry::Entry::Object(obj.into())),
            },
        }
    }
}

// ObjectMap -> proto
impl From<ObjectMap> for archival_proto::ObjectMap {
    fn from(map: ObjectMap) -> Self {
        let entries = map
            .into_iter()
            .map(|(name, object_entry)| archival_proto::object_map::Entry {
                name,
                object: Some(object_entry.into()),
            })
            .collect();
        archival_proto::ObjectMap { entries }
    }
}

// ValuePathComponent -> proto
impl From<ValuePathComponent> for archival_proto::ValuePathComponent {
    fn from(c: ValuePathComponent) -> Self {
        let component = match c {
            ValuePathComponent::Key(k) => archival_proto::value_path_component::Component::Key(k),
            ValuePathComponent::Index(i) => {
                archival_proto::value_path_component::Component::Index(i as u32)
            }
        };
        archival_proto::ValuePathComponent {
            component: Some(component),
        }
    }
}

// ValuePath -> proto
impl From<ValuePath> for archival_proto::ValuePath {
    fn from(vp: ValuePath) -> Self {
        archival_proto::ValuePath {
            path: vp.into_iter().map(|c| c.into()).collect(),
        }
    }
}

// FieldType -> proto
impl From<FieldType> for archival_proto::FieldType {
    fn from(ft: FieldType) -> Self {
        use archival_proto::field_type::Type;
        let r#type = match ft {
            FieldType::String => Type::String(()),
            FieldType::Number => Type::Number(()),
            FieldType::Date => Type::Date(()),
            FieldType::Enum(vals) => Type::Enum(archival_proto::EnumFieldType { values: vals }),
            FieldType::Markdown => Type::Markdown(()),
            FieldType::Boolean => Type::Boolean(()),
            FieldType::Image => Type::Image(()),
            FieldType::Video => Type::Video(()),
            FieldType::Upload => Type::Upload(()),
            FieldType::Audio => Type::Audio(()),
            FieldType::Meta => Type::Meta(()),
            FieldType::Alias(boxed) => {
                let (inner, name) = *boxed;
                Type::Alias(Box::new(archival_proto::AliasType {
                    r#type: Some(Box::new(archival_proto::FieldType::from(inner))),
                    name,
                }))
            }
        };
        archival_proto::FieldType {
            r#type: Some(r#type),
        }
    }
}

// OrderMap<String, FieldType> -> proto FieldsMap
impl From<OrderMap<String, FieldType>> for archival_proto::FieldsMap {
    fn from(map: OrderMap<String, FieldType>) -> Self {
        let fields = map
            .into_iter()
            .map(|(name, ft)| archival_proto::fields_map::Field {
                name,
                r#type: Some(ft.into()),
            })
            .collect();
        archival_proto::FieldsMap { fields }
    }
}

// ObjectDefinition -> proto
impl From<ObjectDefinition> for archival_proto::ObjectDefinition {
    fn from(def: ObjectDefinition) -> Self {
        let fields = if def.fields.is_empty() {
            None
        } else {
            Some(def.fields.into())
        };

        let children = if def.children.is_empty() {
            None
        } else {
            Some(archival_proto::ChildDefinitions {
                children: def
                    .children
                    .into_iter()
                    .map(
                        |(name, child_def)| archival_proto::child_definitions::Child {
                            name,
                            definition: Some(child_def.into()),
                        },
                    )
                    .collect(),
            })
        };

        archival_proto::ObjectDefinition {
            name: def.name,
            fields,
            template: def.template.unwrap_or_default(),
            children,
        }
    }
}

// File
impl From<archival_proto::File> for File {
    fn from(proto: archival_proto::File) -> Self {
        File {
            display_type: archival_proto::DisplayType::try_from(proto.display_type)
                .ok()
                .unwrap_or(archival_proto::DisplayType::Download)
                .into(),
            filename: proto.filename,
            sha: proto.sha,
            mime: proto.mime,
            name: if proto.name.is_empty() {
                None
            } else {
                Some(proto.name)
            },
            description: if proto.description.is_empty() {
                None
            } else {
                Some(proto.description)
            },
        }
    }
}

// MetaValue
impl From<archival_proto::MetaValue> for MetaValue {
    fn from(proto: archival_proto::MetaValue) -> Self {
        match proto.value {
            Some(archival_proto::meta_value::Value::String(s)) => MetaValue::String(s),
            Some(archival_proto::meta_value::Value::Number(n)) => MetaValue::Number(n),
            Some(archival_proto::meta_value::Value::Boolean(b)) => MetaValue::Boolean(b),
            Some(archival_proto::meta_value::Value::DateString(d)) => {
                MetaValue::DateTime(DateTime::from(&d).unwrap_or_else(|_| DateTime::now()))
            }
            Some(archival_proto::meta_value::Value::Array(arr)) => {
                MetaValue::Array(arr.values.into_iter().map(|v| v.into()).collect())
            }
            Some(archival_proto::meta_value::Value::Map(m)) => MetaValue::Map(m.into()),
            None => MetaValue::String(String::new()),
        }
    }
}

// Meta
impl From<archival_proto::Meta> for Meta {
    fn from(proto: archival_proto::Meta) -> Self {
        let mut meta_map = OrderMap::new();
        for entry in proto.entries {
            if let Some(value) = entry.value {
                meta_map.insert(entry.name, value.into());
            }
        }
        Meta(meta_map)
    }
}

// FieldValue
impl From<archival_proto::FieldValue> for FieldValue {
    fn from(proto: archival_proto::FieldValue) -> Self {
        match proto.value {
            Some(archival_proto::field_value::Value::String(s)) => FieldValue::String(s),
            Some(archival_proto::field_value::Value::Enum(e)) => FieldValue::Enum(e),
            Some(archival_proto::field_value::Value::Markdown(m)) => FieldValue::Markdown(m),
            Some(archival_proto::field_value::Value::Number(n)) => FieldValue::Number(n),
            Some(archival_proto::field_value::Value::DateString(d)) => {
                FieldValue::Date(DateTime::from(&d).unwrap_or_else(|_| DateTime::now()))
            }
            Some(archival_proto::field_value::Value::Objects(obj)) => {
                let mut fields = BTreeMap::new();
                for f in obj.fields {
                    fields.insert(
                        f.name,
                        f.value.map(|v| v.into()).unwrap_or(FieldValue::Null),
                    );
                }
                // FieldValue::Objects expects Vec<ObjectValues>
                FieldValue::Objects(vec![fields])
            }
            Some(archival_proto::field_value::Value::Boolean(b)) => FieldValue::Boolean(b),
            Some(archival_proto::field_value::Value::File(f)) => FieldValue::File(f.into()),
            Some(archival_proto::field_value::Value::Meta(m)) => FieldValue::Meta(m.into()),
            Some(archival_proto::field_value::Value::Null(_)) => FieldValue::Null,
            None => FieldValue::Null,
        }
    }
}

// ObjectValues -> BTreeMap<String, RustFieldValue>
impl From<archival_proto::ObjectValues> for BTreeMap<String, FieldValue> {
    fn from(proto: archival_proto::ObjectValues) -> Self {
        proto
            .fields
            .into_iter()
            .map(|f| {
                (
                    f.name,
                    f.value.map(|v| v.into()).unwrap_or(FieldValue::Null),
                )
            })
            .collect()
    }
}

// Object
impl From<archival_proto::Object> for Object {
    fn from(proto: archival_proto::Object) -> Self {
        let order = match proto.order {
            Some(archival_proto::object::Order::None(_)) => None,
            Some(archival_proto::object::Order::Some(o)) => Some(o),
            None => None,
        };
        Object {
            filename: proto.filename,
            object_name: proto.object_name,
            order,
            path: proto.path,
            values: proto.values.map(|v| v.into()).unwrap_or_default(),
        }
    }
}

// ObjectEntry
impl From<archival_proto::ObjectEntry> for ObjectEntry {
    fn from(proto: archival_proto::ObjectEntry) -> Self {
        match proto.entry {
            Some(archival_proto::object_entry::Entry::List(list)) => {
                ObjectEntry::List(list.entries.into_iter().map(|o| o.into()).collect())
            }
            Some(archival_proto::object_entry::Entry::Object(obj)) => {
                ObjectEntry::Object(obj.into())
            }
            None => ObjectEntry::Object(Object {
                filename: String::new(),
                object_name: String::new(),
                order: None,
                path: String::new(),
                values: BTreeMap::new(),
            }),
        }
    }
}

// ObjectMap
impl From<archival_proto::ObjectMap> for ObjectMap {
    fn from(proto: archival_proto::ObjectMap) -> Self {
        proto
            .entries
            .into_iter()
            .map(|e| {
                (
                    e.name,
                    e.object.map(|o| o.into()).unwrap_or_else(|| {
                        ObjectEntry::Object(Object {
                            filename: String::new(),
                            object_name: String::new(),
                            order: None,
                            path: String::new(),
                            values: BTreeMap::new(),
                        })
                    }),
                )
            })
            .collect()
    }
}

// ValuePathComponent
impl From<archival_proto::ValuePathComponent> for ValuePathComponent {
    fn from(proto: archival_proto::ValuePathComponent) -> Self {
        match proto.component {
            Some(archival_proto::value_path_component::Component::Key(k)) => {
                ValuePathComponent::Key(k)
            }
            Some(archival_proto::value_path_component::Component::Index(i)) => {
                ValuePathComponent::Index(i as usize)
            }
            None => ValuePathComponent::Key(String::new()),
        }
    }
}

// ValuePath - convert from proto to rust by building components
impl From<archival_proto::ValuePath> for ValuePath {
    fn from(proto: archival_proto::ValuePath) -> Self {
        let mut path = ValuePath::empty();
        for component_proto in proto.path {
            let component: ValuePathComponent = component_proto.into();
            path = path.append(component);
        }
        path
    }
}

// FieldType - convert from proto struct's oneof field to rust enum
impl From<archival_proto::FieldType> for FieldType {
    fn from(proto: archival_proto::FieldType) -> Self {
        match proto.r#type {
            Some(archival_proto::field_type::Type::String(())) => FieldType::String,
            Some(archival_proto::field_type::Type::Number(())) => FieldType::Number,
            Some(archival_proto::field_type::Type::Date(())) => FieldType::Date,
            Some(archival_proto::field_type::Type::Enum(values)) => FieldType::Enum(values.values),
            Some(archival_proto::field_type::Type::Markdown(())) => FieldType::Markdown,
            Some(archival_proto::field_type::Type::Boolean(())) => FieldType::Boolean,
            Some(archival_proto::field_type::Type::Image(())) => FieldType::Image,
            Some(archival_proto::field_type::Type::Video(())) => FieldType::Video,
            Some(archival_proto::field_type::Type::Upload(())) => FieldType::Upload,
            Some(archival_proto::field_type::Type::Audio(())) => FieldType::Audio,
            Some(archival_proto::field_type::Type::Meta(())) => FieldType::Meta,
            Some(archival_proto::field_type::Type::Alias(alias)) => {
                // Convert AliasType to FieldType - alias.r#type is Option<Box<proto_gen::FieldType>>
                if let Some(boxed_type) = alias.r#type {
                    let converted: FieldType = (*boxed_type).into();
                    FieldType::Alias(Box::new((converted, alias.name)))
                } else {
                    FieldType::String
                }
            }
            None => FieldType::String,
        }
    }
}

// FieldsMap from proto
impl From<archival_proto::FieldsMap> for OrderMap<String, FieldType> {
    fn from(proto: archival_proto::FieldsMap) -> Self {
        let mut map = OrderMap::new();
        for field in proto.fields {
            let field_type = field.r#type.map(|t| t.into()).unwrap_or(FieldType::String);
            map.insert(field.name, field_type);
        }
        map
    }
}

// ObjectDefinition
impl From<archival_proto::ObjectDefinition> for ObjectDefinition {
    fn from(proto: archival_proto::ObjectDefinition) -> Self {
        let fields: OrderMap<String, FieldType> =
            proto.fields.map(|f| f.into()).unwrap_or_default();

        let mut children = OrderMap::new();
        if let Some(cd) = proto.children {
            for child in cd.children {
                let child_def: ObjectDefinition = child
                    .definition
                    .unwrap_or_else(|| archival_proto::ObjectDefinition {
                        name: String::new(),
                        fields: None,
                        template: String::new(),
                        children: None,
                    })
                    .into();
                children.insert(child.name, child_def);
            }
        }

        ObjectDefinition {
            name: proto.name,
            fields,
            template: if proto.template.is_empty() {
                None
            } else {
                Some(proto.template)
            },
            children,
        }
    }
}
