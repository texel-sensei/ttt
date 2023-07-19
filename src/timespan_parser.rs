#![allow(dead_code)] // TODO: Use code

#[derive(Debug, PartialEq, Eq)]
pub enum Type {
    Week,
    Month,
    Year,
    Weekday(u8),
    SpecificMonth(u8),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Token {
    // today, yesterday
    Day(i8),

    Span(Type),

    Last,
    This,
    To,
    Number(i32),

    IsoDate(chrono::NaiveDate),

    Error(String),
}

pub fn tokenize(text: &[impl AsRef<str>]) -> impl Iterator<Item = Token> + '_ {
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

            _ => Error(word.as_ref().to_owned()),
        }
    })
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

        // TODO: fixme
        check(
            "last mOnDaY until 2023-07",
            vec![
                Last,
                Span(Type::Weekday(0)),
                To,
                IsoDate(chrono::NaiveDate::from_ymd_opt(2023, 07, 01).unwrap()),
            ],
        );
    }
}
