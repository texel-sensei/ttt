#![allow(dead_code)] // TODO: Use code

use chrono::{NaiveDateTime, NaiveTime};

use crate::{database::TimeSpan, model::Timestamp};

pub enum ParseError {
    EmptyInput,
    InvalidToken(String),
    UnexpectedToken(String),
}

pub fn parse(text: &[impl AsRef<str>]) -> Result<TimeSpan, ParseError> {
    use ParseError::*;
    let mut tokens = tokenize(text).peekable();
    let Some(token) = tokens.next() else {
        return Err(EmptyInput);
    };
    match token {
        Token::Day(0) if tokens.peek().is_some() => {
            return Err(UnexpectedToken(format!(
                "Unexpected token after 'today' {:?}",
                tokens.peek().unwrap()
            )))
        }
        Token::Day(0) => {
            let now = Timestamp::now();
            return Ok((
                Timestamp::from_naive(NaiveDateTime::new(
                    now.0.date_naive(),
                    NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
                )),
                now,
            ));
        }
        Token::Day(-1) => todo!(),
        Token::Day(i8::MIN..=-2_i8) | Token::Day(1_i8..=i8::MAX) => todo!(),
        Token::Span(_) => todo!(),
        Token::Last => todo!(),
        Token::This => todo!(),
        Token::To => {
            return Err(UnexpectedToken(
                "Timespan cannot start with 'To/Until'".to_owned(),
            ))
        }
        Token::Number(_) => todo!(),
        Token::PartialIsoDate(_, _) => todo!(),
        Token::IsoDate(_) => todo!(),
        Token::Error(e) => return Err(InvalidToken(e)),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Type {
    Week,
    Month,
    Year,
    Weekday(u8),
    SpecificMonth(u8),
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    // today, yesterday
    Day(i8),

    Span(Type),

    Last,
    This,
    To,
    Number(i32),

    PartialIsoDate(i32, u8),
    IsoDate(chrono::NaiveDate),

    Error(String),
}

fn tokenize(text: &[impl AsRef<str>]) -> impl Iterator<Item = Token> + '_ {
    text.iter().map(|word| {
        use Token::*;
        match word.as_ref().to_lowercase().as_ref() {
            "yesterday" => Day(-1),
            "today" => Day(0),
            "last" => Last,
            "this" => This,
            "to" | "until" => To,

            "monday" => Span(Type::Weekday(0)),
            "tuesday" => Span(Type::Weekday(1)),
            "wednesday" => Span(Type::Weekday(2)),
            "thursday" => Span(Type::Weekday(3)),
            "friday" => Span(Type::Weekday(4)),
            "saturday" => Span(Type::Weekday(5)),
            "sunday" => Span(Type::Weekday(6)),

            "january" => Span(Type::SpecificMonth(0)),
            "february" => Span(Type::SpecificMonth(1)),
            "march" => Span(Type::SpecificMonth(2)),
            "april" => Span(Type::SpecificMonth(3)),
            "may" => Span(Type::SpecificMonth(4)),
            "june" => Span(Type::SpecificMonth(5)),
            "july" => Span(Type::SpecificMonth(6)),
            "august" => Span(Type::SpecificMonth(7)),
            "september" => Span(Type::SpecificMonth(8)),
            "october" => Span(Type::SpecificMonth(9)),
            "november" => Span(Type::SpecificMonth(10)),
            "december" => Span(Type::SpecificMonth(11)),

            "week" | "weeks" => Span(Type::Week),
            "month" | "months" => Span(Type::Month),
            "year" | "years" => Span(Type::Year),

            x if x.parse::<i32>().is_ok() => Number(x.parse().unwrap()),

            x if x.parse::<chrono::NaiveDate>().is_ok() => IsoDate(x.parse().unwrap()),

            x if parse_partial_date(x).is_some() => {
                let tmp = parse_partial_date(x).unwrap();
                PartialIsoDate(tmp.0, tmp.1)
            }

            _ => Error(word.as_ref().to_owned()),
        }
    })
}

fn parse_partial_date(date: &str) -> Option<(i32, u8)> {
    let split = date.split_once("-")?;
    Some((split.0.parse().ok()?, split.1.parse().ok()?))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_examples() {
        fn check(text: &str, expected: Vec<Token>) {
            let words: Vec<_> = text.split_whitespace().collect();

            assert_eq!(tokenize(&words).collect::<Vec<_>>(), expected);
        }

        use Token::*;
        check("last tuesday", vec![Last, Span(Type::Weekday(1))]);
        check("this month", vec![This, Span(Type::Month)]);

        check(
            "Foo this 12abc",
            vec![Error("Foo".to_owned()), This, Error("12abc".to_owned())],
        );

        check("to until", vec![To, To]);

        check(
            "last mOnDaY until 2023-07",
            vec![Last, Span(Type::Weekday(0)), To, PartialIsoDate(2023, 7)],
        );

        check(
            "2020-03 to 2023-07-03",
            vec![
                PartialIsoDate(2020, 3),
                To,
                IsoDate(chrono::NaiveDate::from_ymd_opt(2023, 7, 3).unwrap()),
            ],
        );

        check(
            "last year march until this mOnDaY",
            vec![
                Last,
                Span(Type::Year),
                Span(Type::SpecificMonth(2)),
                To,
                This,
                Span(Type::Weekday(0)),
            ],
        );
    }
}
