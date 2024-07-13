use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, tag_no_case, take_until},
    character::complete::multispace0,
    combinator::{map, opt, value},
    multi::fold_many0,
    sequence::{delimited, separated_pair, tuple},
    IResult,
};

#[derive(Debug, PartialEq)]
pub enum Filter<'a> {
    And(Box<Filter<'a>>, Box<Filter<'a>>),
    Or(Box<Filter<'a>>, Box<Filter<'a>>),
    Not(Box<Filter<'a>>),
    Term(&'a str),
}

/// Parse a filter query that follows the general pattern of Google's advanced search syntax:
///
///   * Adjacent terms are ANDed together
///   * Terms with an OR between them are ORed together
///   * Terms with a `-` in front of them are negated
///
/// Concretely:
///
///   * `"boop jones"` searches for `"boop"` _and_ `"jones"`
///   * `"boop -jones"` searches for `"boop"` and _not_ `"jones"`
///   * `"boop or jones"` searches for `"boop"` _or_ `"jones"`
pub fn parse_filter(input: &str) -> Option<Filter> {
    let Some((remaining, filter)) = filter(input).ok() else {
        return None;
    };
    if remaining.len() != 0 {
        return None;
    }
    filter
}

fn term(input: &str) -> IResult<&str, Filter> {
    delimited(
        multispace0,
        map(
            tuple((opt(value((), tag("-"))), alt((quoted_term, unquoted_term)))),
            |(negated, term_str)| match negated {
                Some(()) => Filter::Not(Filter::Term(term_str).into()),
                None => Filter::Term(term_str),
            },
        ),
        multispace0,
    )(input)
}

fn or(input: &str) -> IResult<&str, Filter> {
    map(separated_pair(term, tag_no_case("or"), term), |(a, b)| {
        Filter::Or(a.into(), b.into())
    })(input)
}

fn filter(input: &str) -> IResult<&str, Option<Filter>> {
    fold_many0(
        alt((or, term)),
        || None,
        |acc: Option<Filter>, item: Filter| match acc {
            Some(other) => Some(Filter::And(other.into(), item.into())),
            None => Some(item),
        },
    )(input)
}

fn unquoted_term(input: &str) -> IResult<&str, &str> {
    is_not(" \t\r\n")(input)
}

fn quoted_term(input: &str) -> IResult<&str, &str> {
    delimited(tag("\""), take_until("\""), tag("\""))(input)
}

#[cfg(test)]
mod tests {
    use crate::filter_parser::{parse_filter, Filter};

    #[test]
    fn test_parse_filter_works() {
        assert_eq!(parse_filter(""), None);
        assert_eq!(parse_filter("hi"), Some(Filter::Term("hi")));
        assert_eq!(
            parse_filter("hi there"),
            Some(Filter::And(
                Filter::Term("hi").into(),
                Filter::Term("there").into(),
            ))
        );
        assert_eq!(
            parse_filter("hi     \"there bub\""),
            Some(Filter::And(
                Filter::Term("hi").into(),
                Filter::Term("there bub").into(),
            ))
        );
        assert_eq!(
            parse_filter("hi there bub"),
            Some(Filter::And(
                Filter::And(Filter::Term("hi").into(), Filter::Term("there").into()).into(),
                Filter::Term("bub").into(),
            ))
        );
        assert_eq!(
            parse_filter("hi OR there"),
            Some(Filter::Or(
                Filter::Term("hi").into(),
                Filter::Term("there").into(),
            ))
        );
        assert_eq!(
            parse_filter("hi there OR bub"),
            Some(Filter::And(
                Filter::Term("hi").into(),
                Filter::Or(Filter::Term("there").into(), Filter::Term("bub").into()).into(),
            ))
        );
        assert_eq!(
            parse_filter("hi -there"),
            Some(Filter::And(
                Filter::Term("hi").into(),
                Filter::Not(Filter::Term("there").into()).into(),
            ))
        );
        assert_eq!(
            parse_filter("hi -\"there bub\""),
            Some(Filter::And(
                Filter::Term("hi").into(),
                Filter::Not(Filter::Term("there bub").into()).into(),
            ))
        );
    }
}
