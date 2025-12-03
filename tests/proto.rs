#[cfg(feature = "proto")]
mod proto_tests {
    use std::{collections::HashMap, fs};

    use archival::{archival_proto, object::Object, FieldValue};
    use prost::Message;

    #[test]
    fn restore_from_proto_11_25_25() {
        let files = HashMap::from([
            (
                "subpage/hello",
                fs::read("tests/fixtures/proto/subpage-hello.pb").unwrap(),
            ),
            (
                "section/first",
                fs::read("tests/fixtures/proto/section-first.pb").unwrap(),
            ),
        ]);
        let mut objects: HashMap<String, Object> = HashMap::new();
        for (key, content) in files {
            objects.insert(
                key.to_string(),
                archival_proto::Object::decode(&*content).unwrap().into(),
            );
        }

        let subpage_hello = objects.get("subpage/hello").unwrap();

        assert_eq!(subpage_hello.order, None);
        let name = subpage_hello.values.get("name").unwrap();
        assert_eq!(*name, FieldValue::String("hello".to_string()));

        let section_first = objects.get("section/first").unwrap();
        let name = section_first.values.get("name").unwrap();
        assert_eq!(*name, FieldValue::String("Some Content".to_string()));
        let body = section_first.values.get("body").unwrap();
        assert_eq!(
            *body,
            FieldValue::Markdown("Here is some *content*\n".to_string())
        );
    }
}
