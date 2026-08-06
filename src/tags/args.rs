use liquid_core::model::KString;
use liquid_core::parser::{TagToken, TryMatchToken};
use liquid_core::Expression;
use liquid_core::Result;
use liquid_core::TagTokenIter;

/// Parses the `key: value` argument list shared by `include` and `layout`.
///
/// Shopify separates the partial name from its first argument with a comma
/// (`{% include 'a', k: v %}`) where stdlib liquid uses only whitespace
/// (`{% include 'a' k: v %}`). Both are accepted, as is a trailing comma.
pub(crate) fn parse_vars(arguments: &mut TagTokenIter<'_>) -> Result<Vec<(KString, Expression)>> {
    let first = arguments.next();
    parse_vars_from(first, arguments)
}

/// [`parse_vars`] for callers that have already pulled the token following the
/// partial name (to check for a `with`/`for` clause).
pub(crate) fn parse_vars_from<'a>(
    first: Option<TagToken<'a>>,
    arguments: &mut TagTokenIter<'a>,
) -> Result<Vec<(KString, Expression)>> {
    let mut vars: Vec<(KString, Expression)> = Vec::new();
    let mut token = match first {
        Some(t) => match t.expect_str(",") {
            TryMatchToken::Matches(()) => arguments.next(),
            TryMatchToken::Fails(t) => Some(t),
        },
        None => None,
    };

    while let Some(t) = token {
        let id = t.expect_identifier().into_result()?.to_owned();

        arguments
            .expect_next("\":\" expected.")?
            .expect_str(":")
            .into_result_custom_msg("expected \":\" to be used for the assignment")?;

        vars.push((
            id.into(),
            arguments
                .expect_next("expected value")?
                .expect_value()
                .into_result()?,
        ));

        token = match arguments.next() {
            Some(comma) => match comma.expect_str(",") {
                TryMatchToken::Matches(()) => arguments.next(),
                TryMatchToken::Fails(t) => {
                    return t
                        .raise_custom_error("`,` is needed to separate variables")
                        .into_err()
                }
            },
            None => None,
        };
    }

    arguments.expect_nothing()?;

    Ok(vars)
}
