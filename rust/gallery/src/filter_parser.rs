use nom::{
    branch::alt, bytes::complete::{is_not, tag, take_until}, character::complete::multispace1, combinator::map, multi::separated_list0, sequence::delimited, IResult
};

#[derive(Debug, Clone, PartialEq)]
pub enum FilterToken<'a> {
    Or,
    Not,
    Term(&'a str),
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
    FilterToken::Term(value)
}

fn quoted_term(input: &str) -> IResult<&str, FilterToken> {
    delimited(tag("\""), map(take_until("\""), str_to_term), tag("\""))(input)
}

pub fn filter_token(input: &str) -> IResult<&str, FilterToken> {
    alt((or, not, quoted_term, term))(input)
}

pub fn filter_tokens(input: &str) -> IResult<&str, Vec<FilterToken>> {
    separated_list0(multispace1, filter_token)(input)
}

#[cfg(test)]
mod tests {
    use crate::filter_parser::{filter_token, filter_tokens, FilterToken};

    #[test]
    fn test_filter_token_works() {
        assert_eq!(filter_token("hi"), Ok(("", FilterToken::Term("hi"))));
        assert_eq!(filter_token("hi-there"), Ok(("", FilterToken::Term("hi-there"))));
        assert_eq!(filter_token("hi there"), Ok((" there", FilterToken::Term("hi"))));
        assert_eq!(filter_token("\"hi there\""), Ok(("", FilterToken::Term("hi there"))));
        assert_eq!(filter_token("\"hi or - there\""), Ok(("", FilterToken::Term("hi or - there"))));
        assert_eq!(filter_token("-"), Ok(("", FilterToken::Not)));
        assert_eq!(filter_token("or"), Ok(("", FilterToken::Or)));
    }

    #[test]
    fn test_filter_tokens_works() {
        assert_eq!(filter_tokens("hi"), Ok(("", vec![FilterToken::Term("hi")])));
        assert_eq!(filter_tokens("hi bub"), Ok(("", vec![FilterToken::Term("hi"), FilterToken::Term("bub")])));
        assert_eq!(filter_tokens("hi \"bub sup\""), Ok(("", vec![FilterToken::Term("hi"), FilterToken::Term("bub sup")])));
    }
}
