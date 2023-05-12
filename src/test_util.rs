#[cfg(test)]
pub mod test_util {
    pub struct TestData<'a> {
        pub definition_toml: &'a str,
        pub object_toml: &'a str,
        pub page_content: &'a str,
    }
    pub fn get_test_data() -> TestData<'static> {
        TestData {
            definition_toml: "",
            object_toml: "",
            page_content: "",
        }
    }
}
