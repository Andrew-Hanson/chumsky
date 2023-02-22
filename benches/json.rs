#![feature(test, array_methods)]

extern crate test;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Str(String),
    Num(f64),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsonZero<'a> {
    Null,
    Bool(bool),
    Str(&'a [u8]),
    Num(f64),
    Array(Vec<JsonZero<'a>>),
    Object(Vec<(&'a [u8], JsonZero<'a>)>),
}

static JSON: &'static [u8] = include_bytes!("sample.json");

fn bench_json(c: &mut Criterion) {
    c.bench_function("json_nom", {
        move |b| b.iter(|| black_box(nom::json(black_box(JSON)).unwrap()))
    });

    c.bench_function("json_nom_str", {
        let s = black_box(std::str::from_utf8(JSON).unwrap());
        move |b| b.iter(|| black_box(nom_str::json(s).unwrap()))
    });

    c.bench_function("json_chumsky_zero_copy", {
        use ::chumsky::zero_copy::prelude::*;
        let json = chumsky_zero_copy::json();
        move |b| {
            b.iter(|| {
                black_box(json.parse(black_box(JSON)))
                    .into_result()
                    .unwrap()
            })
        }
    });

    c.bench_function("json_chumsky_zero_copy_check", {
        use ::chumsky::zero_copy::prelude::*;
        let json = chumsky_zero_copy::json();
        move |b| {
            b.iter(|| {
                assert!(black_box(json.check(black_box(JSON)))
                    .into_errors()
                    .is_empty())
            })
        }
    });

    c.bench_function("json_chumsky_zero_copy_str", {
        use ::chumsky::zero_copy::prelude::*;
        let json = chumsky_zero_copy_str::json();
        let s = black_box(std::str::from_utf8(JSON).unwrap());
        move |b| {
            b.iter(|| {
                black_box(json.parse(s))
                    .into_result()
                    .unwrap()
            })
        }
    });

    c.bench_function("json_chumsky_zero_copy_check_str", {
        use ::chumsky::zero_copy::prelude::*;
        let json = chumsky_zero_copy_str::json();
        let s = black_box(std::str::from_utf8(JSON).unwrap());
        move |b| {
            b.iter(|| {
                assert!(black_box(json.check(s))
                    .into_errors()
                    .is_empty())
            })
        }
    });

    c.bench_function("json_serde_json", {
        use serde_json::{from_slice, Value};
        move |b| b.iter(|| black_box(from_slice::<Value>(black_box(JSON)).unwrap()))
    });

    c.bench_function("json_pom", {
        let json = pom::json();
        move |b| b.iter(|| black_box(json.parse(black_box(JSON)).unwrap()))
    });

    c.bench_function("json_pest", {
        let json = black_box(std::str::from_utf8(JSON).unwrap());
        move |b| b.iter(|| black_box(pest::parse(json).unwrap()))
    });

    c.bench_function("json_chumsky", {
        use ::chumsky::prelude::*;
        let json = chumsky::json();
        move |b| b.iter(|| black_box(json.parse(black_box(JSON)).unwrap()))
    });
}

criterion_group!(benches, bench_json);
criterion_main!(benches);

mod chumsky_zero_copy {
    use chumsky::zero_copy::prelude::*;

    use super::JsonZero;
    use std::str;

    pub fn json<'a>() -> impl Parser<'a, [u8], JsonZero<'a>> {
        recursive(|value| {
            let digits = one_of(b'0'..=b'9').repeated();

            let int = one_of(b'1'..=b'9')
                .then(one_of(b'0'..=b'9').repeated())
                .ignored()
                .or(just(b'0').ignored())
                .ignored();

            let frac = just(b'.').then(digits.clone());

            let exp = one_of(b"eE")
                .then(one_of(b"+-").or_not())
                .then(digits.clone());

            let number = just(b'-')
                .or_not()
                .then(int)
                .then(frac.or_not())
                .then(exp.or_not())
                .map_slice(|bytes| str::from_utf8(bytes).unwrap().parse().unwrap())
                .boxed();

            let escape = just(b'\\').ignore_then(choice((
                just(b'\\'),
                just(b'/'),
                just(b'"'),
                just(b'b').to(b'\x08'),
                just(b'f').to(b'\x0C'),
                just(b'n').to(b'\n'),
                just(b'r').to(b'\r'),
                just(b't').to(b'\t'),
            )));

            let string = none_of(b"\\\"")
                .or(escape)
                .repeated()
                .slice()
                .delimited_by(just(b'"'), just(b'"'))
                .boxed();

            let array = value
                .clone()
                .separated_by(just(b','))
                .collect()
                .padded()
                .delimited_by(just(b'['), just(b']'))
                .boxed();

            let member = string.clone().then_ignore(just(b':').padded()).then(value);
            let object = member
                .clone()
                .separated_by(just(b',').padded())
                .collect()
                .padded()
                .delimited_by(just(b'{'), just(b'}'))
                .boxed();

            choice((
                just(b"null").to(JsonZero::Null),
                just(b"true").to(JsonZero::Bool(true)),
                just(b"false").to(JsonZero::Bool(false)),
                number.map(JsonZero::Num),
                string.map(JsonZero::Str),
                array.map(JsonZero::Array),
                object.map(JsonZero::Object),
            ))
            .padded()
        })
        .then(end())
        .map(|(json, _)| json)
    }
}

mod chumsky_zero_copy_str {
    use chumsky::zero_copy::prelude::*;

    use super::JsonZero;
    use std::str;

    pub fn json<'a>() -> impl Parser<'a, str, JsonZero<'a>> {
        recursive(|value| {
            let digits = one_of('0'..='9').repeated();

            let int = one_of('1'..='9')
                .then(one_of('0'..='9').repeated())
                .ignored()
                .or(just('0').ignored())
                .ignored();

            let frac = just('.').then(digits.clone());

            let exp = one_of("eE")
                .then(one_of("+-").or_not())
                .then(digits.clone());

            let number = just('-')
                .or_not()
                .then(int)
                .then(frac.or_not())
                .then(exp.or_not())
                .map_slice(|s: &str| s.parse().unwrap())
                .boxed();

            let escape = just('\\').ignore_then(choice((
                just('\\'),
                just('/'),
                just('"'),
                just('b').to('\x08'),
                just('f').to('\x0C'),
                just('n').to('\n'),
                just('r').to('\r'),
                just('t').to('\t'),
            )));

            let string = none_of("\\\"")
                .or(escape)
                .repeated()
                .map_slice(|s: &str| s.as_bytes())
                .delimited_by(just('"'), just('"'))
                .boxed();

            let array = value
                .clone()
                .separated_by(just(','))
                .collect()
                .padded()
                .delimited_by(just('['), just(']'))
                .boxed();

            let member = string.clone().then_ignore(just(':').padded()).then(value);
            let object = member
                .clone()
                .separated_by(just(',').padded())
                .collect()
                .padded()
                .delimited_by(just('{'), just('}'))
                .boxed();

            choice((
                just("null").to(JsonZero::Null),
                just("true").to(JsonZero::Bool(true)),
                just("false").to(JsonZero::Bool(false)),
                number.map(JsonZero::Num),
                string.map(JsonZero::Str),
                array.map(JsonZero::Array),
                object.map(JsonZero::Object),
            ))
            .padded()
        })
        .then(end())
        .map(|(json, _)| json)
    }
}

mod chumsky {
    use chumsky::{error::Cheap, prelude::*};

    use super::Json;
    use std::str;

    pub fn json() -> impl Parser<u8, Json, Error = Cheap<u8>> {
        recursive(|value| {
            let frac = just(b'.').chain(text::digits(10));

            let exp = one_of(b"eE")
                .ignore_then(just(b'+').or(just(b'-')).or_not())
                .chain(text::digits(10));

            let number = just(b'-')
                .or_not()
                .chain(text::int(10))
                .chain(frac.or_not().flatten())
                .chain::<u8, _, _>(exp.or_not().flatten())
                .map(|bytes| str::from_utf8(&bytes.as_slice()).unwrap().parse().unwrap());

            let escape = just(b'\\').ignore_then(choice((
                just(b'\\'),
                just(b'/'),
                just(b'"'),
                just(b'b').to(b'\x08'),
                just(b'f').to(b'\x0C'),
                just(b'n').to(b'\n'),
                just(b'r').to(b'\r'),
                just(b't').to(b'\t'),
            )));

            let string = just(b'"')
                .ignore_then(filter(|c| *c != b'\\' && *c != b'"').or(escape).repeated())
                .then_ignore(just(b'"'))
                .map(|bytes| String::from_utf8(bytes).unwrap());

            let array = value
                .clone()
                .separated_by(just(b',').padded())
                .padded()
                .delimited_by(just(b'['), just(b']'))
                .map(Json::Array);

            let member = string.then_ignore(just(b':').padded()).then(value);
            let object = member
                .separated_by(just(b',').padded())
                .padded()
                .delimited_by(just(b'{'), just(b'}'))
                .collect::<Vec<(String, Json)>>()
                .map(Json::Object);

            choice((
                just(b"null").to(Json::Null),
                just(b"true").to(Json::Bool(true)),
                just(b"false").to(Json::Bool(false)),
                number.map(Json::Num),
                string.map(Json::Str),
                array,
                object,
            ))
            .padded()
        })
        .then_ignore(end())
    }
}

mod pom {
    use pom::parser::*;
    use pom::Parser;

    use super::Json;
    use std::str::{self, FromStr};

    fn space() -> Parser<u8, ()> {
        one_of(b" \t\r\n").repeat(0..).discard()
    }

    fn number() -> Parser<u8, f64> {
        let integer = one_of(b"123456789") - one_of(b"0123456789").repeat(0..) | sym(b'0');
        let frac = sym(b'.') + one_of(b"0123456789").repeat(1..);
        let exp = one_of(b"eE") + one_of(b"+-").opt() + one_of(b"0123456789").repeat(1..);
        let number = sym(b'-').opt() + integer + frac.opt() + exp.opt();
        number
            .collect()
            .convert(str::from_utf8)
            .convert(|s| f64::from_str(&s))
    }

    fn string() -> Parser<u8, String> {
        let special_char = sym(b'\\')
            | sym(b'/')
            | sym(b'"')
            | sym(b'b').map(|_| b'\x08')
            | sym(b'f').map(|_| b'\x0C')
            | sym(b'n').map(|_| b'\n')
            | sym(b'r').map(|_| b'\r')
            | sym(b't').map(|_| b'\t');
        let escape_sequence = sym(b'\\') * special_char;
        let string = sym(b'"') * (none_of(b"\\\"") | escape_sequence).repeat(0..) - sym(b'"');
        string.convert(String::from_utf8)
    }

    fn array() -> Parser<u8, Vec<Json>> {
        let elems = list(call(value), sym(b',') * space());
        sym(b'[') * space() * elems - sym(b']')
    }

    fn object() -> Parser<u8, Vec<(String, Json)>> {
        let member = string() - space() - sym(b':') - space() + call(value);
        let members = list(member, sym(b',') * space());
        let obj = sym(b'{') * space() * members - sym(b'}');
        obj.map(|members| members.into_iter().collect::<Vec<_>>())
    }

    fn value() -> Parser<u8, Json> {
        (seq(b"null").map(|_| Json::Null)
            | seq(b"true").map(|_| Json::Bool(true))
            | seq(b"false").map(|_| Json::Bool(false))
            | number().map(|num| Json::Num(num))
            | string().map(|text| Json::Str(text))
            | array().map(|arr| Json::Array(arr))
            | object().map(|obj| Json::Object(obj)))
            - space()
    }

    pub fn json() -> Parser<u8, Json> {
        space() * value() - end()
    }
}

mod nom {
    use nom::{
        branch::alt,
        bytes::complete::{escaped, tag, take_while},
        character::complete::{char, digit0, digit1, none_of, one_of},
        combinator::{cut, map, opt, recognize, value as to},
        error::ParseError,
        multi::separated_list0,
        sequence::{preceded, separated_pair, terminated, tuple},
        IResult,
    };

    use super::JsonZero;
    use std::str;

    fn space<'a, E: ParseError<&'a [u8]>>(i: &'a [u8]) -> IResult<&'a [u8], &'a [u8], E> {
        take_while(|c| b" \t\r\n".contains(&c))(i)
    }

    fn number<'a, E: ParseError<&'a [u8]>>(i: &'a [u8]) -> IResult<&'a [u8], f64, E> {
        map(
            recognize(tuple((
                opt(char('-')),
                alt((
                    to((), tuple((one_of("123456789"), digit0))),
                    to((), char('0')),
                )),
                opt(tuple((char('.'), digit1))),
                opt(tuple((one_of("eE"), opt(one_of("+-")), cut(digit1)))),
            ))),
            |bytes| str::from_utf8(bytes).unwrap().parse::<f64>().unwrap(),
        )(i)
    }

    fn string<'a, E: ParseError<&'a [u8]>>(i: &'a [u8]) -> IResult<&'a [u8], &'a [u8], E> {
        preceded(
            char('"'),
            cut(terminated(
                escaped(none_of("\\\""), '\\', one_of("\\/\"bfnrt")),
                char('"'),
            )),
        )(i)
    }

    fn array<'a, E: ParseError<&'a [u8]>>(i: &'a [u8]) -> IResult<&'a [u8], Vec<JsonZero>, E> {
        preceded(
            char('['),
            cut(terminated(
                separated_list0(preceded(space, char(',')), value),
                preceded(space, char(']')),
            )),
        )(i)
    }

    fn member<'a, E: ParseError<&'a [u8]>>(
        i: &'a [u8],
    ) -> IResult<&'a [u8], (&'a [u8], JsonZero), E> {
        separated_pair(
            preceded(space, string),
            cut(preceded(space, char(':'))),
            value,
        )(i)
    }

    fn object<'a, E: ParseError<&'a [u8]>>(
        i: &'a [u8],
    ) -> IResult<&'a [u8], Vec<(&'a [u8], JsonZero)>, E> {
        preceded(
            char('{'),
            cut(terminated(
                separated_list0(preceded(space, char(',')), member),
                preceded(space, char('}')),
            )),
        )(i)
    }

    fn value<'a, E: ParseError<&'a [u8]>>(i: &'a [u8]) -> IResult<&'a [u8], JsonZero, E> {
        preceded(
            space,
            alt((
                to(JsonZero::Null, tag("null")),
                to(JsonZero::Bool(true), tag("true")),
                to(JsonZero::Bool(false), tag("false")),
                map(number, JsonZero::Num),
                map(string, JsonZero::Str),
                map(array, JsonZero::Array),
                map(object, JsonZero::Object),
            )),
        )(i)
    }

    fn root<'a, E: ParseError<&'a [u8]>>(i: &'a [u8]) -> IResult<&'a [u8], JsonZero, E> {
        terminated(value, space)(i)
    }

    pub fn json<'a>(i: &'a [u8]) -> IResult<&'a [u8], JsonZero, (&'a [u8], nom::error::ErrorKind)> {
        root(i)
    }
}

mod nom_str {
    use nom::{
        branch::alt,
        bytes::complete::{escaped, tag, take_while},
        character::complete::{char, digit0, digit1, none_of, one_of},
        combinator::{cut, map, opt, recognize, value as to},
        error::ParseError,
        multi::separated_list0,
        sequence::{preceded, separated_pair, terminated, tuple},
        IResult,
    };

    use super::JsonZero;
    use std::str;

    fn space<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
        take_while(|c| " \t\r\n".contains(*&c))(i)
    }

    fn number<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, f64, E> {
        map(
            recognize(tuple((
                opt(char('-')),
                alt((
                    to((), tuple((one_of("123456789"), digit0))),
                    to((), char('0')),
                )),
                opt(tuple((char('.'), digit1))),
                opt(tuple((one_of("eE"), opt(one_of("+-")), cut(digit1)))),
            ))),
            |s: &str| s.parse::<f64>().unwrap(),
        )(i)
    }

    fn string<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a [u8], E> {
        preceded(
            char('"'),
            cut(terminated(
                map(escaped(none_of("\\\""), '\\', one_of("\\/\"bfnrt")), |s: &str| s.as_bytes()),
                char('"'),
            )),
        )(i)
    }

    fn array<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, Vec<JsonZero>, E> {
        preceded(
            char('['),
            cut(terminated(
                separated_list0(preceded(space, char(',')), value),
                preceded(space, char(']')),
            )),
        )(i)
    }

    fn member<'a, E: ParseError<&'a str>>(
        i: &'a str,
    ) -> IResult<&'a str, (&'a [u8], JsonZero), E> {
        separated_pair(
            preceded(space, string),
            cut(preceded(space, char(':'))),
            value,
        )(i)
    }

    fn object<'a, E: ParseError<&'a str>>(
        i: &'a str,
    ) -> IResult<&'a str, Vec<(&'a [u8], JsonZero)>, E> {
        preceded(
            char('{'),
            cut(terminated(
                separated_list0(preceded(space, char(',')), member),
                preceded(space, char('}')),
            )),
        )(i)
    }

    fn value<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, JsonZero, E> {
        preceded(
            space,
            alt((
                to(JsonZero::Null, tag("null")),
                to(JsonZero::Bool(true), tag("true")),
                to(JsonZero::Bool(false), tag("false")),
                map(number, JsonZero::Num),
                map(string, JsonZero::Str),
                map(array, JsonZero::Array),
                map(object, JsonZero::Object),
            )),
        )(i)
    }

    fn root<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, JsonZero, E> {
        terminated(value, space)(i)
    }

    pub fn json<'a>(i: &'a str) -> IResult<&'a str, JsonZero, (&'a str, nom::error::ErrorKind)> {
        root(i)
    }
}

mod pest {
    use super::JsonZero;

    use pest::{error::Error, Parser};

    #[derive(pest_derive::Parser)]
    #[grammar = "benches/json.pest"]
    struct JsonParser;

    pub fn parse(file: &str) -> Result<JsonZero, Error<Rule>> {
        let json = JsonParser::parse(Rule::json, file)?.next().unwrap();

        use pest::iterators::Pair;

        fn parse_value(pair: Pair<Rule>) -> JsonZero {
            match pair.as_rule() {
                Rule::object => JsonZero::Object(
                    pair.into_inner()
                        .map(|pair| {
                            let mut inner_rules = pair.into_inner();
                            let name = inner_rules
                                .next()
                                .unwrap()
                                .into_inner()
                                .next()
                                .unwrap()
                                .as_str();
                            let value = parse_value(inner_rules.next().unwrap());
                            (name.as_bytes(), value)
                        })
                        .collect(),
                ),
                Rule::array => JsonZero::Array(pair.into_inner().map(parse_value).collect()),
                Rule::string => {
                    JsonZero::Str(pair.into_inner().next().unwrap().as_str().as_bytes())
                }
                Rule::number => JsonZero::Num(pair.as_str().parse().unwrap()),
                Rule::boolean => JsonZero::Bool(pair.as_str().parse().unwrap()),
                Rule::null => JsonZero::Null,
                Rule::json
                | Rule::EOI
                | Rule::pair
                | Rule::value
                | Rule::inner
                | Rule::char
                | Rule::WHITESPACE => unreachable!(),
            }
        }

        Ok(parse_value(json))
    }
}
