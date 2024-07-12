use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_until},
    combinator::map,
    sequence::delimited,
    IResult,
};

#[derive(Debug, Clone, PartialEq)]
pub enum FilterToken {
    Or,
    Not,
    Term(String),
}

fn or(input: &str) -> IResult<&str, FilterToken> {
    nom::combinator::value(FilterToken::Or, tag("or"))(input)
}

fn not(input: &str) -> IResult<&str, FilterToken> {
    nom::combinator::value(FilterToken::Not, tag("-"))(input)
}

fn term(input: &str) -> IResult<&str, FilterToken> {
    nom::combinator::map(is_not(" \t\r\n"), str_to_term)(input)
}

fn str_to_term(value: &str) -> FilterToken {
    FilterToken::Term(value.to_string())
}

fn quoted_term(input: &str) -> IResult<&str, FilterToken> {
    delimited(tag("\""), map(take_until("\""), str_to_term), tag("\""))(input)
}

pub fn filter_token(input: &str) -> IResult<&str, FilterToken> {
    alt((or, not, quoted_term, term))(input)
}

#[cfg(test)]
mod tests {
    use crate::filter_parser::{filter_token, FilterToken};

    #[test]
    fn test_it_works() {
        assert_eq!(filter_token("hi"), Ok(("", FilterToken::Term("hi".to_string()))));
        assert_eq!(filter_token("hi-there"), Ok(("", FilterToken::Term("hi-there".to_string()))));
        assert_eq!(filter_token("hi there"), Ok((" there", FilterToken::Term("hi".to_string()))));
        assert_eq!(filter_token("\"hi there\""), Ok(("", FilterToken::Term("hi there".to_string()))));
        assert_eq!(filter_token("\"hi or - there\""), Ok(("", FilterToken::Term("hi or - there".to_string()))));
        assert_eq!(filter_token("-"), Ok(("", FilterToken::Not)));
        assert_eq!(filter_token("or"), Ok(("", FilterToken::Or)));
    }
}
