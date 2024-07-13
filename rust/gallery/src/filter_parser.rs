use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, tag_no_case, take_until},
    character::complete::multispace1,
    combinator::{map, opt, value},
    multi::separated_list0,
    sequence::{delimited, tuple},
    IResult,
};

#[derive(Debug, PartialEq)]
pub enum Filter<'a> {
    And(Box<Filter<'a>>, Box<Filter<'a>>),
    Or(Box<Filter<'a>>, Box<Filter<'a>>),
    Not(Box<Filter<'a>>),
    Term(&'a str),
}

#[derive(Debug, Clone, PartialEq)]
enum FilterToken<'a> {
    Or,
    Term(&'a str),
    NegatedTerm(&'a str),
}

fn or(input: &str) -> IResult<&str, FilterToken> {
    nom::combinator::value(FilterToken::Or, tag_no_case("or"))(input)
}

fn unquoted_term(input: &str) -> IResult<&str, &str> {
    is_not(" \t\r\n")(input)
}

fn quoted_term(input: &str) -> IResult<&str, &str> {
    delimited(tag("\""), take_until("\""), tag("\""))(input)
}

fn term(input: &str) -> IResult<&str, FilterToken> {
    map(
        tuple((opt(value((), tag("-"))), alt((quoted_term, unquoted_term)))),
        |(negated, term_str)| match negated {
            Some(()) => FilterToken::NegatedTerm(term_str),
            None => FilterToken::Term(term_str),
        },
    )(input)
}

fn filter_token(input: &str) -> IResult<&str, FilterToken> {
    alt((or, term))(input)
}

fn filter_tokens(input: &str) -> IResult<&str, Vec<FilterToken>> {
    separated_list0(multispace1, filter_token)(input)
}

pub fn parse_filter(input: &str) -> Option<Filter> {
    let Some((remaining, tokens)) = filter_tokens(input).ok() else {
        return None;
    };
    if remaining.len() != 0 {
        return None;
    }
    // It's possible that this could be implemented using a separate nom parser that operates
    // on FilterTokens and yields Filter? Regardless, the following implementation is kind of
    // gross and convoluted, but the syntax we're parsing is simple enough that it'll do for now.
    let mut current: Option<Filter> = None;
    let mut use_or = false;
    for token in tokens {
        let next = match token {
            FilterToken::Or => {
                use_or = true;
                continue;
            }
            FilterToken::Term(value) => Filter::Term(value),
            FilterToken::NegatedTerm(value) => Filter::Not(Box::new(Filter::Term(value))),
        };
        let next_current = match current.take() {
            Some(current) => {
                if use_or {
                    Filter::Or(Box::new(current), Box::new(next))
                } else {
                    Filter::And(Box::new(current), Box::new(next))
                }
            }
            None => next,
        };
        current = Some(next_current);
    }
    current
}

#[cfg(test)]
mod tests {
    use crate::filter_parser::{filter_token, filter_tokens, parse_filter, Filter, FilterToken};

    #[test]
    fn test_parse_filter_works() {
        assert_eq!(parse_filter("hi"), Some(Filter::Term("hi")));
        assert_eq!(
            parse_filter("hi there"),
            Some(Filter::And(
                Filter::Term("hi").into(),
                Filter::Term("there").into(),
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
            parse_filter("hi -there"),
            Some(Filter::And(
                Filter::Term("hi").into(),
                Filter::Not(Filter::Term("there").into()).into(),
            ))
        );
    }

    #[test]
    fn test_filter_token_works() {
        assert_eq!(filter_token("hi"), Ok(("", FilterToken::Term("hi"))));
        assert_eq!(
            filter_token("hi-there"),
            Ok(("", FilterToken::Term("hi-there")))
        );
        assert_eq!(
            filter_token("hi there"),
            Ok((" there", FilterToken::Term("hi")))
        );
        assert_eq!(
            filter_token("\"hi there\""),
            Ok(("", FilterToken::Term("hi there")))
        );
        assert_eq!(
            filter_token("\"hi or - there\""),
            Ok(("", FilterToken::Term("hi or - there")))
        );
        assert_eq!(
            filter_token("-boop"),
            Ok(("", FilterToken::NegatedTerm("boop")))
        );
        assert_eq!(filter_token("or"), Ok(("", FilterToken::Or)));
    }

    #[test]
    fn test_filter_tokens_works() {
        assert_eq!(filter_tokens("hi"), Ok(("", vec![FilterToken::Term("hi")])));
        assert_eq!(
            filter_tokens("hi bub"),
            Ok(("", vec![FilterToken::Term("hi"), FilterToken::Term("bub")]))
        );
        assert_eq!(
            filter_tokens("hi -bub"),
            Ok((
                "",
                vec![FilterToken::Term("hi"), FilterToken::NegatedTerm("bub")]
            ))
        );
        assert_eq!(
            filter_tokens("hi \"bub sup\""),
            Ok((
                "",
                vec![FilterToken::Term("hi"), FilterToken::Term("bub sup")]
            ))
        );
    }
}
