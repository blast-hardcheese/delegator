use crate::translate::Language;

use nom::branch::alt;
use nom::bytes::complete::{escaped, is_not, tag};
use nom::character::complete::{alpha1, alphanumeric1, char, one_of, space0};
use nom::combinator::{opt, recognize};
use nom::multi::{many0_count, separated_list0, separated_list1};
use nom::sequence::{delimited, pair, preceded};
use nom::{IResult, Parser};
use wson;

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
    let mut focus = opt(preceded(delimited(space0, char('|'), space0), parse_thunk));

    let (input, key) = leader(input)?;
    let (input, proj) = focus(input)?;

    let term = match proj {
        None => Language::at(key),
        Some(rest) => Language::at(key).map(rest),
    };
    Ok((input, term))
}

fn convert_wson_value(json: wson::Value) -> serde_json::Value {
    match json {
        wson::Value::Null => serde_json::Value::Null,
        wson::Value::True => serde_json::Value::Bool(true),
        wson::Value::False => serde_json::Value::Bool(false),
        wson::Value::Number(wson::number::Number::PositiveInteger(n)) => {
            serde_json::Value::Number(n.into())
        }
        wson::Value::Number(wson::number::Number::NegativeInteger(n)) => {
            serde_json::Value::Number(n.into())
        }
        wson::Value::Number(wson::number::Number::Float(n)) => {
            let num = serde_json::Number::from_f64(n).unwrap();
            serde_json::Value::Number(num)
        }
        wson::Value::String(s) => serde_json::Value::String(s),
        wson::Value::Array(vec) => {
            serde_json::Value::Array(vec.into_iter().map(convert_wson_value).collect())
        }
        wson::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, convert_wson_value(v)))
                .collect(),
        ),
    }
}

fn parse_const(input: &str) -> IResult<&str, Language> {
    let leader = tag("const(");
    let follower = char(')');

    let (input, _) = leader(input)?;
    let (input, value) = wson::json(input)?;
    let (input, _) = follower(input)?;

    Ok((input, Language::Const(convert_wson_value(value))))
}

fn parse_default(input: &str) -> IResult<&str, Language> {
    let leader = tag("default(");
    let follower = char(')');

    let (input, _) = leader(input)?;
    let (input, prog) = parse_thunk(input)?;
    let (input, _) = follower(input)?;

    Ok((input, Language::Default(Box::new(prog))))
}

fn parse_flatten(input: &str) -> IResult<&str, Language> {
    let leader = tag("flatten");

    let (input, _) = leader(input)?;

    Ok((input, Language::Flatten))
}

fn parse_identity(input: &str) -> IResult<&str, Language> {
    let leader = char('.');

    let (input, _) = leader(input)?;

    Ok((input, Language::Identity))
}

fn parse_map(input: &str) -> IResult<&str, Language> {
    delimited(
        tag("map("),
        delimited(
            space0,
            Parser::map(Parser::map(parse_thunk, Box::new), Language::Array),
            space0,
        ),
        char(')'),
    )(input)
}

fn parse_object(input: &str) -> IResult<&str, Language> {
    let parse_entry = pair(
        Parser::map(quoted, String::from),
        preceded(delimited(space0, char(':'), space0), parse_thunk),
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

fn parse_get(input: &str) -> IResult<&str, Language> {
    let (input, key) = delimited(
        delimited(space0, tag("get(\""), space0),
        identifier,
        tag("\")"),
    )(input)?;
    Ok((input, Language::get(key)))
}

fn parse_set(input: &str) -> IResult<&str, Language> {
    let (input, key) = delimited(
        delimited(space0, tag("set(\""), space0),
        identifier,
        tag("\")"),
    )(input)?;
    Ok((input, Language::set(key)))
}

fn parse_thunk(input: &str) -> IResult<&str, Language> {
    parse_at(input)
        .or_else(|_| parse_map(input))
        .or_else(|_| parse_object(input))
        .or_else(|_| parse_get(input))
        .or_else(|_| parse_set(input))
        .or_else(|_| parse_const(input))
        .or_else(|_| parse_default(input))
        .or_else(|_| parse_flatten(input))
        .or_else(|_| parse_identity(input))
}

pub fn parse_language(input: &str) -> IResult<&str, Language> {
    let (input, matched) =
        separated_list1(delimited(space0, tag(","), space0), parse_thunk)(input)?;
    match matched.as_slice() {
        [only] => Ok((input, only.clone())),
        rest => Ok((input, Language::Splat(rest.to_vec()))),
    }
}

#[test]
fn test_parse_at() {
    let (input, lang) = parse_at(".foo").unwrap();
    assert_eq!(input, "");
    assert_eq!(lang, Language::at("foo"));
}

#[test]
fn test_parse_focus() {
    let prog = ".foo | .bar";
    let expected = Language::at("foo").map(Language::at("bar"));
    let (input, result) = parse_at(prog).unwrap();
    assert_eq!(input, "");
    assert_eq!(result, expected);
}

#[test]
fn test_parse_map() {
    let (input, lang) = parse_language(".foo").unwrap();
    assert_eq!(input, "");
    assert_eq!(lang, Language::at("foo"));
}

#[test]
fn test_parse_object() {
    let prog = r#"{ "foo" : map(.foo) , "bar" : .bar }"#;
    let expected = vec![
        (String::from("foo"), Language::array(Language::at("foo"))),
        (String::from("bar"), Language::at("bar")),
    ];

    let (input, lang) = parse_language(prog).unwrap();
    assert_eq!(input, "");
    assert_eq!(lang, Language::Object(expected));
}

#[test]
fn test_parse_set_get() {
    let prog = r#".foo | set("foo"), { "bar": .bar, "foo": get("foo") }"#;
    let expected = Language::Splat(vec![
        Language::at("foo").map(Language::set("foo")),
        Language::Object(vec![
            (String::from("bar"), Language::at("bar")),
            (String::from("foo"), Language::Get(String::from("foo"))),
        ]),
    ]);

    let (input, entries) = parse_language(prog).unwrap();
    assert_eq!(input, "");
    assert_eq!(entries, expected);
}
