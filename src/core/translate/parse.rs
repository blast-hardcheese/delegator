use crate::translate::Language;

use nom::branch::alt;
use nom::bytes::complete::{escaped, is_not, tag};
use nom::character::complete::{alpha1, alphanumeric1, char, one_of, space0};
use nom::combinator::{opt, recognize};
use nom::multi::{many0_count, separated_list0};
use nom::sequence::{delimited, pair, preceded};
use nom::{IResult, Parser};

fn identifier(input: &str) -> IResult<&str, &str> {
    recognize(pair(
        alt((alpha1, tag("_"))),
        many0_count(alt((alphanumeric1, tag("_")))),
    ))(input)
}

fn quoted(input: &str) -> IResult<&str, &str> {
    delimited(
        char('"'),
        escaped(is_not("\\\""), '\\', one_of(r#""\"#)),
        char('"'),
    )(input)
}

fn parse_at(input: &str) -> IResult<&str, Language> {
    let mut leader = preceded(char('.'), identifier);
    let mut focus = opt(preceded(
        delimited(space0, char('|'), space0),
        parse_language,
    ));

    let (input, key) = leader(input)?;
    let (input, proj) = focus(input)?;

    let name = String::from(key);
    let term = match proj {
        None => Language::At(name),
        Some(rest) => Language::Focus(name, Box::new(rest)),
    };
    Ok((input, term))
}

fn parse_map(input: &str) -> IResult<&str, Language> {
    delimited(
        tag("map("),
        delimited(
            space0,
            Parser::map(Parser::map(parse_language, Box::new), Language::Array),
            space0,
        ),
        char(')'),
    )(input)
}

fn parse_object(input: &str) -> IResult<&str, Language> {
    let parse_entry = pair(
        Parser::map(quoted, String::from),
        preceded(delimited(space0, char(':'), space0), parse_language),
    );

    delimited(
        delimited(space0, char('{'), space0),
        Parser::map(
            separated_list0(delimited(space0, char(','), space0), parse_entry),
            Language::Object,
        ),
        delimited(space0, char('}'), space0),
    )(input)
}

pub fn parse_language(input: &str) -> IResult<&str, Language> {
    parse_at(input)
        .or_else(|_| parse_map(input))
        .or_else(|_| parse_object(input))
}

#[test]
fn test_parse_at() {
    if let Ok((input, Language::At(name))) = parse_at(".foo") {
        assert_eq!(input, "");
        assert_eq!(name, "foo");
    }
}

#[test]
fn test_parse_focus() {
    let prog = ".foo | .bar";
    let expected = Language::Focus(
        String::from("foo"),
        Box::new(Language::At(String::from("bar"))),
    );
    if let Ok((input, result)) = parse_at(prog) {
        assert_eq!(input, "");
        assert_eq!(result, expected);
    }
}

#[test]
fn test_parse_map() {
    if let Ok((input, Language::At(name))) = parse_language(".foo") {
        assert_eq!(input, "");
        assert_eq!(name, "foo");
    }
}

#[test]
fn test_parse_object() {
    let prog = r#"{ "foo" : map(.foo) , "bar" : .bar }"#;
    let expected = vec![
        (
            String::from("foo"),
            Language::Array(Box::new(Language::At(String::from("foo")))),
        ),
        (String::from("bar"), Language::At(String::from("bar"))),
    ];

    if let Ok((input, Language::Object(entries))) = parse_language(prog) {
        assert_eq!(input, "");
        assert_eq!(entries, expected);
    }
}