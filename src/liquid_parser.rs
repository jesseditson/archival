pub fn get() -> liquid::Parser {
    liquid::ParserBuilder::with_stdlib().build().unwrap()
}
