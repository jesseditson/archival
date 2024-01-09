use crate::tags::layout::LayoutTag;

pub fn get() -> liquid::Parser {
    liquid::ParserBuilder::with_stdlib()
        .tag(LayoutTag)
        .build()
        .unwrap()
}
