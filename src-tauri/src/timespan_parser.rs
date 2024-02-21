#![allow(dead_code)] // TODO: Use code

use std::{cmp::min, iter::Peekable};

use chrono::{Datelike, Days, Months};

use crate::model::{TimeSpan, TimeSpanError, Timestamp};

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    EmptyInput,
    InvalidToken(String),
    UnexpectedToken(String),
    MissingEnd,

    EndBeforeStart(Timestamp, Timestamp),

    /// The time span would exceed the representable time.
    OutOfRange,

    /// Nobody seems to agree when "this tuesday" is.
    LanguageIsComplicated,
}

impl From<TimeSpanError> for ParseError {
    fn from(value: TimeSpanError) -> Self {
        match value {
            TimeSpanError::EndBeforeStart(start, end) => ParseError::EndBeforeStart(start, end),
        }
    }
}

pub struct Context {
    pub now: Timestamp,
}

pub fn parse(text: &[impl AsRef<str>], context: &Context) -> Result<TimeSpan, ParseError> {
    let mut tokens = tokenize(text).peekable();

    let initial_timespan = parse_simple_timespan(&mut tokens, context)?;

    match tokens.next() {
        None => Ok(initial_timespan),
        Some(Token::To) => {
            let full_timespan =
                initial_timespan.extend(parse_simple_timespan(&mut tokens, context)?)?;
            if tokens.peek().is_some() {
                // TODO(texel, 2023-11-21): return original lexeme
                return Err(ParseError::UnexpectedToken(format!("{:?}", tokens.peek())));
            }
            Ok(full_timespan)
        }
        Some(other_token) => Err(ParseError::UnexpectedToken(format!("{:?}", other_token))),
    }
}

/// Parses a timespan without the token "To", e.g. "last week".
fn parse_simple_timespan(
    tokens: &mut Peekable<impl Iterator<Item = Token>>,
    context: &Context,
) -> Result<TimeSpan, ParseError> {
    match tokens.next().ok_or(ParseError::EmptyInput)? {
        Token::Day(0) if tokens.peek().is_some() => Err(ParseError::UnexpectedToken(format!(
            "Unexpected token after 'today' {:?}",
            tokens.peek().unwrap()
        ))),
        Token::Day(offset) if offset <= 0 => {
            let offset = Days::new(-offset as u64);
            let begin = context.now.at_midnight() - offset;
            Ok(TimeSpan::new(
                begin,
                min(context.now, begin + Days::new(1)),
            )?)
        }
        Token::To => Err(ParseError::UnexpectedToken(
            "Timespan cannot start with 'To/Until'".to_owned(),
        )),
        Token::This if matches!(tokens.peek(), Some(Token::Span(_))) => {
            let Some(Token::Span(span)) = tokens.next() else {
                unreachable!()
            };
            Ok(parse_span(span, context, true)?)
        }
        Token::Last if matches!(tokens.peek(), Some(Token::Span(_))) => {
            let Some(Token::Span(span)) = tokens.next() else {
                unreachable!()
            };
            Ok(parse_span(span, context, false)?)
        }

        // parse e.g. "last 3 weeks"
        Token::Last if matches!(tokens.peek(), Some(Token::Number(_))) => {
            // let Some(Token::Number(number)) = tokens.next() else {
            //     unreachable!()
            // };
            // let Some(token) = tokens.next() else {
            //     return Err(ParseError::MissingEnd);
            // };
            // let Token::Span(span @ (Type::Week | Type::Month | Type::Year)) = token else {
            //     return Err(ParseError::UnexpectedToken(
            //         format!("Unexpected '{token:?}' after 'last {number}', expected 'weeks', 'months' or 'years'")
            //     ));
            // };
            // let mut duration = parse_span(span, context, false)?;
            // match span {
            //     Type::Week => {
            //         *duration.start_mut() = duration.start() - Days::new(7*number as u64);
            //     },
            //     Type::Month => {
            //         *duration.start_mut() = duration.start() - Months::new(number as u32 - 1);
            //     },
            //     Type::Year => todo!(),
            //     _ => unreachable!(),
            // }
            // Ok(duration)
            todo!()
        }
        Token::Span(Type::Weekday(day)) => {
            let now = context.now;
            let mut start = now.at_midnight()
                - Days::new(now.0.weekday().num_days_from_monday() as u64)
                + Days::new(day as u64);
            if start > now {
                start = start - Days::new(7);
            }
            let end = start + Days::new(1);

            Ok(TimeSpan::new(start, end)?)
        }
        Token::Span(Type::SpecificMonth(month)) => {
            let now = context.now;
            let mut start: Timestamp = now
                .at_midnight()
                .0
                .with_day(1)
                .unwrap()
                .with_month0(month as u32)
                .unwrap()
                .into();

            if start > now {
                start = start - Months::new(12);
            }
            let end = start + Months::new(1);

            Ok(TimeSpan::new(start, end)?)
        }
        other => Err(ParseError::UnexpectedToken(format!(
            "Unexpected token '{other:?}'"
        ))),
    }
}

fn parse_span(span: Type, context: &Context, is_current: bool) -> Result<TimeSpan, ParseError> {
    let timespan = match span {
        Type::Week => {
            let now = context.now;
            let start =
                now.at_midnight() - Days::new(now.0.weekday().num_days_from_monday() as u64);
            let end = start + Days::new(7);

            TimeSpan::new(start, end)
        }
        Type::Month => {
            let start = context.now.at_midnight().0.with_day(1).unwrap();
            let end = start + Months::new(1);

            TimeSpan::new(start, end)
        }
        Type::Year => {
            let start = context
                .now
                .at_midnight()
                .0
                .with_day(1)
                .unwrap()
                .with_month(1)
                .unwrap();
            let end = start + Months::new(12);

            TimeSpan::new(start, end)
        }
        Type::Weekday(_) => {
            return Err(ParseError::LanguageIsComplicated);
        }
        Type::SpecificMonth(_) => return Err(ParseError::LanguageIsComplicated),
    }?;

    Ok(match (&span, is_current) {
        (_, true) => timespan,
        (Type::Week | Type::Weekday(_), false) => {
            let start = timespan.start() - Days::new(7);
            let end = timespan.end() - Days::new(7);

            TimeSpan::new(start, end)?
        }
        (Type::Month, false) => {
            let start = timespan.start() - Months::new(1);
            let end = timespan.end() - Months::new(1);

            TimeSpan::new(start, end)?
        }
        (Type::Year | Type::SpecificMonth(_), false) => {
            let start = timespan.start() - Months::new(12);
            let end = timespan.end() - Months::new(12);

            TimeSpan::new(start, end)?
        }
    })
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Type {
    Week,
    Month,
    Year,

    /// Day of the week, zero based
    Weekday(u8),

    /// Month of the year, zero based
    SpecificMonth(u8),
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    /// A point in time relative to "Now". For example "today" = `Day(0)` and "yesterday" =
    /// `Day(-1)`.
    Day(i8),

    Span(Type),

    Last,
    This,
    To,
    Number(u32),

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

            // TODO(texel, 2024-02-21): include days? last 3 days
            "week" | "weeks" => Span(Type::Week),
            "month" | "months" => Span(Type::Month),
            "year" | "years" => Span(Type::Year),

            x if x.parse::<u32>().is_ok() => Number(x.parse().unwrap()),

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
    let split = date.split_once('-')?;
    Some((split.0.parse().ok()?, split.1.parse().ok()?))
}

#[cfg(test)]
mod test {
    use chrono::NaiveDate;

    use super::*;

    #[test]
    fn test_tokenize_examples() {
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

    fn new_timestamp(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> Timestamp {
        Timestamp::from_naive(
            NaiveDate::from_ymd_opt(y, m, d)
                .unwrap()
                .and_hms_opt(h, min, s)
                .unwrap(),
        )
    }

    #[test]
    fn test_parse_today() {
        let context = Context {
            now: new_timestamp(2023, 10, 25, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2023, 10, 25, 0, 0, 0),
            new_timestamp(2023, 10, 25, 12, 33, 17),
        )
        .unwrap();
        assert_eq!(parse(&["today"], &context).unwrap(), expected);
    }

    #[test]
    fn test_parse_yesterday() {
        let context = Context {
            now: new_timestamp(2023, 10, 25, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2023, 10, 24, 0, 0, 0),
            new_timestamp(2023, 10, 25, 0, 0, 0),
        )
        .unwrap();
        assert_eq!(parse(&["yesterday"], &context).unwrap(), expected);
    }

    #[test]
    fn test_parse_simple_range() {
        let context = Context {
            now: new_timestamp(2023, 10, 25, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2023, 10, 24, 0, 0, 0),
            new_timestamp(2023, 10, 25, 12, 33, 17),
        )
        .unwrap();
        assert_eq!(
            parse(&["yesterday", "until", "today"], &context).unwrap(),
            expected
        );
    }

    #[test]
    fn test_parse_simple_range_with_garbage_at_the_end_fails() {
        let context = Context {
            now: new_timestamp(2023, 10, 25, 12, 33, 17),
        };

        assert!(matches!(
            parse(&["yesterday", "until", "today", "to"], &context),
            Err(ParseError::UnexpectedToken(_))
        ));
    }

    #[test]
    fn test_this_today_is_not_allowed() {
        let context = Context {
            now: new_timestamp(2023, 10, 25, 12, 33, 17),
        };

        assert!(matches!(
            parse(&["this", "today"], &context),
            Err(ParseError::UnexpectedToken(_))
        ));
    }

    #[test]
    fn test_parse_this_week() {
        let context = Context {
            now: new_timestamp(2023, 10, 25, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2023, 10, 23, 0, 0, 0),
            new_timestamp(2023, 10, 30, 0, 0, 0),
        )
        .unwrap();
        assert_eq!(parse(&["this", "week"], &context).unwrap(), expected);
    }

    #[test]
    fn test_parse_last_week() {
        let context = Context {
            now: new_timestamp(2023, 10, 25, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2023, 10, 16, 0, 0, 0),
            new_timestamp(2023, 10, 23, 0, 0, 0),
        )
        .unwrap();
        assert_eq!(parse(&["last", "week"], &context).unwrap(), expected);
    }

    #[test]
    fn test_parse_last_month() {
        let context = Context {
            now: new_timestamp(2023, 10, 25, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2023, 9, 1, 0, 0, 0),
            new_timestamp(2023, 10, 1, 0, 0, 0),
        )
        .unwrap();
        assert_eq!(parse(&["last", "month"], &context).unwrap(), expected);
    }

    #[test]
    fn test_parse_this_month() {
        let context = Context {
            now: new_timestamp(2023, 10, 25, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2023, 10, 1, 0, 0, 0),
            new_timestamp(2023, 11, 1, 0, 0, 0),
        )
        .unwrap();
        assert_eq!(parse(&["this", "month"], &context).unwrap(), expected);
    }

    #[test]
    fn test_parse_this_year() {
        let context = Context {
            now: new_timestamp(2023, 10, 25, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2023, 1, 1, 0, 0, 0),
            new_timestamp(2024, 1, 1, 0, 0, 0),
        )
        .unwrap();
        assert_eq!(parse(&["this", "year"], &context).unwrap(), expected);
    }

    #[test]
    fn test_parse_last_year() {
        let context = Context {
            now: new_timestamp(2024, 2, 29, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2023, 1, 1, 0, 0, 0),
            new_timestamp(2024, 1, 1, 0, 0, 0),
        )
        .unwrap();
        assert_eq!(parse(&["last", "year"], &context).unwrap(), expected);
    }

    #[test]
    fn test_parse_wednesday() {
        let context = Context {
            // saturday
            now: new_timestamp(2024, 2, 24, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2024, 2, 21, 0, 0, 0),
            new_timestamp(2024, 2, 22, 0, 0, 0),
        )
        .unwrap();
        assert_eq!(parse(&["wednesday"], &context).unwrap(), expected);
    }

    #[test]
    fn test_parse_wednesday_when_today_is_wednesday() {
        let context = Context {
            // wednesday
            now: new_timestamp(2024, 2, 21, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2024, 2, 21, 0, 0, 0),
            new_timestamp(2024, 2, 22, 0, 0, 0),
        )
        .unwrap();
        assert_eq!(parse(&["wednesday"], &context).unwrap(), expected);
    }

    #[test]
    fn test_parse_complicated_language() {
        let context = Context {
            // wednesday
            now: new_timestamp(2024, 2, 21, 12, 33, 17),
        };

        assert_eq!(
            parse(&["this", "thursday"], &context),
            Err(ParseError::LanguageIsComplicated)
        );
        assert_eq!(
            parse(&["last", "thursday"], &context),
            Err(ParseError::LanguageIsComplicated)
        );
    }

    #[test]
    fn test_parse_this_thursday() {
        let context = Context {
            // wednesday
            now: new_timestamp(2024, 2, 21, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2024, 2, 15, 0, 0, 0),
            new_timestamp(2024, 2, 16, 0, 0, 0),
        )
        .unwrap();
        assert_eq!(parse(&["thursday"], &context).unwrap(), expected);
    }

    #[test]
    fn test_parse_march() {
        let context = Context {
            now: new_timestamp(2024, 3, 21, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2024, 3, 1, 0, 0, 0),
            new_timestamp(2024, 4, 1, 0, 0, 0),
        )
        .unwrap();
        assert_eq!(parse(&["march"], &context).unwrap(), expected);
    }

    #[test]
    fn test_parse_april_returns_last_years_april() {
        let context = Context {
            now: new_timestamp(2024, 3, 21, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2023, 4, 1, 0, 0, 0),
            new_timestamp(2023, 5, 1, 0, 0, 0),
        )
        .unwrap();
        assert_eq!(parse(&["april"], &context).unwrap(), expected);
    }

    #[test]
    fn test_parse_more_complicated_thing() {
        let context = Context {
            now: new_timestamp(2024, 3, 21, 12, 33, 17),
        };

        let expected = TimeSpan::new(
            new_timestamp(2023, 4, 1, 0, 0, 0),
            new_timestamp(2024, 3, 21, 0, 0, 0),
        )
        .unwrap();
        assert_eq!(
            parse(&["april", "to", "yesterday"], &context).unwrap(),
            expected
        );
        //assert_eq!(parse(&["april", "to", "2023-03-20"], &context).unwrap(), expected);

        // assert_eq!(
        //     parse(&["last", "3", "weeks"], &context).unwrap(),
        //     TimeSpan::new(
        //         new_timestamp(2023, 4, 1, 0, 0, 0),
        //         new_timestamp(2024, 3, 21, 12, 33, 17),
        //     ).unwrap());
    }
}
