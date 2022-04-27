use clap::{CommandFactory, ErrorKind, Parser};
use csv;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::one_of,
    combinator::{fail, recognize},
    multi::{many1, separated_list1},
    sequence::{preceded, separated_pair, terminated},
    IResult,
};
use serde_json;
use std::collections::HashMap;
use std::convert;
use std::io;

/* parsers for target */

/// Parse a natural number.
fn natural(input: &str) -> IResult<&str, u8> {
    let (input, value) = recognize(many1(one_of("0123456789")))(input)?;
    let v: u8 = value.parse().unwrap();
    if v < 1 {
        fail(input)
    } else {
        Ok((input, v))
    }
}

/// Selected portion.
#[derive(Debug, PartialEq, Clone)]
pub enum Range {
    /// Column.
    /// e.g. 3
    Single(u8),
    /// Left limited range.
    /// `Left(x)` selects all columns from x to the last number.
    /// e.g. 4-
    Left(u8),
    /// Right limited range.
    /// `Right(x)` selects all columns from 1 to the x.
    /// e.g. -5
    Right(u8),
    /// Inclusive interval.
    /// e.g. 7-9
    Interval(u8, u8),
}

impl Range {
    fn ends(&self) -> (usize, usize) {
        match self {
            Range::Single(x) => (*x as usize, *x as usize + 1),
            Range::Left(x) => (*x as usize, usize::MAX),
            Range::Right(x) => (0, *x as usize + 1),
            Range::Interval(x, y) => (*x as usize, *y as usize + 1),
        }
    }
}

fn single(input: &str) -> IResult<&str, Range> {
    let (input, value) = natural(input)?;
    Ok((input, Range::Single(value - 1)))
}

fn left(input: &str) -> IResult<&str, Range> {
    let (input, limit) = terminated(natural, tag("-"))(input)?;
    Ok((input, Range::Left(limit - 1)))
}

fn right(input: &str) -> IResult<&str, Range> {
    let (input, limit) = preceded(tag("-"), natural)(input)?;
    Ok((input, Range::Right(limit - 1)))
}

fn interval(input: &str) -> IResult<&str, Range> {
    let (input, (left_limit, right_limit)) = separated_pair(natural, tag("-"), natural)(input)?;
    Ok((input, Range::Interval(left_limit - 1, right_limit - 1)))
}

fn range(input: &str) -> IResult<&str, Range> {
    alt((interval, right, left, single))(input)
}

#[derive(Debug, PartialEq, Clone)]
pub struct Target {
    pub ranges: Vec<Range>,
}

fn target(input: &str) -> IResult<&str, Target> {
    let (input, ranges) = separated_list1(tag(","), range)(input)?;
    Ok((input, Target { ranges }))
}

/* field selector */

impl Target {
    /// Cut out selected portions of the row.
    fn select(&self, row: impl TargetRow) -> ResultRow {
        let rlen = row.len();
        let it = self.ranges.iter().map(|x| x.ends());
        let it = it.map(|(left, right)| {
            let v: Vec<usize> = (0..rlen).filter(|i| *i >= left && *i < right).collect();
            v
        });
        let mut v = Vec::new();
        for indexes in it {
            for idx in indexes {
                v.push(row.get(idx).unwrap().to_owned());
            }
        }
        ResultRow(v)
    }
}

trait TargetRow {
    fn get(&self, i: usize) -> Option<&str>;
    fn len(&self) -> usize;
}

#[derive(Debug, Clone)]
struct ResultRow(Vec<String>);

impl convert::From<ResultRow> for Vec<String> {
    fn from(from: ResultRow) -> Vec<String> {
        from.0
    }
}

#[derive(Debug)]
struct RecordRow(csv::StringRecord);

impl RecordRow {
    fn new(record: csv::StringRecord) -> RecordRow {
        RecordRow(record)
    }
}

impl TargetRow for RecordRow {
    fn get(&self, i: usize) -> Option<&str> {
        self.0.get(i)
    }
    fn len(&self) -> usize {
        self.0.len()
    }
}

/* IO */

struct Input(
    Box<dyn Iterator<Item = Result<RecordRow, csv::Error>>>,
    Option<RecordRow>,
);

/// Read values separated by `delimiter` from stdin.
fn read_csv(delimiter: u8, need_headers: bool) -> Input {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(need_headers)
        .delimiter(delimiter)
        .from_reader(io::stdin());
    let headers = if need_headers {
        match reader.headers() {
            Err(_) => None,
            Ok(v) => {
                if v.is_empty() {
                    None
                } else {
                    Some(RecordRow::new(v.clone()))
                }
            }
        }
    } else {
        None
    };
    Input(
        Box::new(reader.into_records().map(|x| x.map(RecordRow::new))),
        headers,
    )
}

struct ResultWriter {
    json: bool,
    headers: Option<ResultRow>,
}

impl ResultWriter {
    fn new(json: bool, target: &Target, headers: Option<RecordRow>) -> ResultWriter {
        ResultWriter {
            json,
            headers: headers.map(|x| target.select(x)),
        }
    }
    /// Write the result into stdout, or the error into stderr.
    fn write(&self, r: Result<ResultRow, csv::Error>) {
        match r {
            Err(err) => eprintln!("{}", err),
            Ok(r) => {
                if self.json {
                    self.write_json(r)
                } else {
                    self.write_csv(r)
                }
            }
        }
    }
    fn write_csv(&self, r: ResultRow) {
        let v: Vec<_> = r.into();
        let v = v.join(",");
        println!("{}", v);
    }
    fn write_json(&self, r: ResultRow) {
        if self.headers.is_none() {
            self.write_json_array(r)
        } else {
            self.write_json_object(r)
        }
    }
    fn write_json_array(&self, r: ResultRow) {
        let v: Vec<_> = r.into();
        match serde_json::to_string(&v) {
            Err(err) => eprintln!("{}", err),
            Ok(r) => println!("{}", r),
        }
    }
    fn write_json_object(&self, r: ResultRow) {
        let row: Vec<_> = r.into();
        let headers: Vec<_> = self.headers.as_ref().unwrap().clone().into();
        let zipped = row.iter().zip(headers.iter());
        let mut map = HashMap::new();
        for (v, h) in zipped {
            map.insert(h, v);
        }
        match serde_json::to_string(&map) {
            Err(err) => eprintln!("{}", err),
            Ok(r) => println!("{}", r),
        }
    }
}

fn main() {
    let cli = Cli::parse();

    // validate command line arguments
    if cli.delimiter.len_utf8() > 1 {
        let mut cmd = Cli::command();
        cmd.error(ErrorKind::InvalidValue, "Delimiter expect 1 byte character")
            .exit();
    }

    match target(&cli.target) {
        Err(err) => {
            let mut cmd = Cli::command();
            cmd.error(ErrorKind::InvalidValue, format!("Invalid target {}", err))
                .exit();
        }
        // normal case
        Ok(("", tgt)) => {
            let t = &tgt;
            let input = read_csv(cli.delimiter as u8, cli.header);
            let writer = ResultWriter::new(cli.json, t, input.1);
            input
                .0
                .map(|x| x.map(|z| t.select(z)))
                .for_each(|x| writer.write(x));
        }
        Ok((x, tgt)) => {
            let mut cmd = Cli::command();
            cmd.error(
                ErrorKind::InvalidValue,
                format!("Invalid target, parsed as {:?} but remaining {}", tgt, x),
            )
            .exit();
        }
    }
}

#[derive(Parser, Debug)]
/// Cut out selected portions of each line of csv from stdin.
struct Cli {
    /// Selected portions.
    ///
    /// Single:
    /// ```
    /// ❯ (echo 'a,b,c';echo '2,3,4';echo '11,12,13') | csvcut -f 1
    /// a
    /// 2
    /// 11
    /// ```
    /// Left limit:
    /// ```
    /// ❯ (echo 'a,b,c';echo '2,3,4';echo '11,12,13') | csvcut -f 2-
    /// b,c
    /// 3,4
    /// 12,13
    /// ```
    /// Right limit:
    /// ```
    /// ❯ (echo 'a,b,c';echo '2,3,4';echo '11,12,13') | csvcut -f -2
    /// a,b
    /// 2,3
    /// 11,12
    /// ```
    /// Interval:
    /// ```
    /// ❯ (echo 'a,b,c,d';echo '1,2,3,4';echo '11,12,13,14') | csvcut -f 2-3
    /// b,c
    /// 2,3
    /// 12,13
    /// ```
    /// Single + Right:
    /// ```
    /// ❯ (echo 'a,b,c,d';echo '1,2,3,4';echo '11,12,13,14') | csvcut -f 1,3-
    /// a,c,d
    /// 1,3,4
    /// 11,13,14
    /// ```
    /// Single + Right, ignore headers:
    /// ```
    /// ❯ (echo 'a,b,c,d';echo '1,2,3,4';echo '11,12,13,14') | csvcut -f 1,3- --header
    /// 1,3,4
    /// 11,13,14
    /// ```
    #[clap(short = 'f', long, allow_hyphen_values = true, verbatim_doc_comment)]
    target: String,
    /// Use DELIMITER as the field delimiter character instead of the ','.
    #[clap(short, long, default_value = ",")]
    delimiter: char,
    /// Print results as json.
    ///
    /// e.g.
    /// ```
    /// ❯ (echo 'a,b,c';echo '2,3,4';echo '11,12,13') | csvcut -f 2 --json
    /// ["b"]
    /// ["3"]
    /// ["12"]
    /// ❯ (echo 'a,b,c';echo '2,3,4';echo '11,12,13') | csvcut -f 2 --json --header
    /// {"b":"3"}
    /// {"b":"12"}
    /// ```
    #[clap(short, long, verbatim_doc_comment)]
    json: bool,
    /// Read or ignore headers.  See --json and --target.
    #[clap(long)]
    header: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    struct VRow<'a>(Vec<&'a str>);
    impl<'a> TargetRow for VRow<'a> {
        fn get(&self, i: usize) -> Option<&str> {
            self.0.get(i).map(|x| *x)
        }
        fn len(&self) -> usize {
            self.0.len()
        }
    }

    macro_rules! test_select {
        ($name:ident, $target:expr, $row:expr, $want:expr) => {
            #[test]
            fn $name() {
                let got = $target.select(VRow($row));
                let got: Vec<String> = got.into();
                assert_eq!($want, got);
            }
        };
    }

    fn empty_strs() -> Vec<&'static str> {
        Vec::new()
    }

    // error
    test_select!(
        select_none,
        Target { ranges: vec![] },
        vec!["top"],
        empty_strs()
    );
    test_select!(
        select_from_none,
        Target {
            ranges: vec![Range::Single(0)],
        },
        empty_strs(),
        empty_strs()
    );
    // Range::Single
    test_select!(
        select_single,
        Target {
            ranges: vec![Range::Single(0)],
        },
        vec!["top"],
        vec!["top"]
    );
    test_select!(
        select_single_failure,
        Target {
            ranges: vec![Range::Single(1)],
        },
        vec!["top"],
        empty_strs()
    );
    // Range::Left
    test_select!(
        select_left_over_right,
        Target {
            ranges: vec![Range::Left(2)],
        },
        vec!["top"],
        empty_strs()
    );
    test_select!(
        select_left_center,
        Target {
            ranges: vec![Range::Left(1)],
        },
        vec!["top", "two"],
        vec!["two"]
    );
    test_select!(
        select_left,
        Target {
            ranges: vec![Range::Left(0)],
        },
        vec!["top", "two"],
        vec!["top", "two"]
    );
    // Range::Right
    test_select!(
        select_right_over_right,
        Target {
            ranges: vec![Range::Right(3)],
        },
        vec!["top", "two"],
        vec!["top", "two"]
    );
    test_select!(
        select_right_center,
        Target {
            ranges: vec![Range::Right(0)],
        },
        vec!["top", "two"],
        vec!["top"]
    );
    test_select!(
        select_right,
        Target {
            ranges: vec![Range::Right(1)],
        },
        vec!["top", "two"],
        vec!["top", "two"]
    );
    // Range::Interval
    test_select!(
        select_interval_out_of_bounds,
        Target {
            ranges: vec![Range::Interval(2, 3)],
        },
        vec!["top"],
        empty_strs()
    );
    test_select!(
        select_interval_right_out_of_bounds,
        Target {
            ranges: vec![Range::Interval(0, 3)],
        },
        vec!["top"],
        vec!["top"]
    );
    test_select!(
        select_interval_single,
        Target {
            ranges: vec![Range::Interval(1, 1)],
        },
        vec!["top", "two"],
        vec!["two"]
    );
    test_select!(
        select_interval_negative,
        Target {
            ranges: vec![Range::Interval(1, 0)],
        },
        vec!["top", "two"],
        empty_strs()
    );
    // Range::Single + Range::Interval
    test_select!(
        select_single_and_interval,
        Target {
            ranges: vec![Range::Single(0), Range::Interval(3, 4)],
        },
        vec!["0", "1", "2", "3", "4", "5"],
        vec!["0", "3", "4"]
    );
    test_select!(
        select_single_and_interval_crossing,
        Target {
            ranges: vec![Range::Single(3), Range::Interval(2, 4)],
        },
        vec!["0", "1", "2", "3", "4", "5"],
        vec!["3", "2", "3", "4"]
    );

    macro_rules! test_target {
        ($name:ident, $input:expr, $want:expr) => {
            #[test]
            fn $name() {
                let got = target($input);
                assert_eq!($want, got);
            }
        };
    }

    test_target!(
        parse_target_single,
        "2",
        Ok((
            "",
            Target {
                ranges: vec![Range::Single(1)]
            }
        ))
    );

    test_target!(
        parse_target_interval_and_single,
        "2-3,4",
        Ok((
            "",
            Target {
                ranges: vec![Range::Interval(1, 2), Range::Single(3)]
            }
        ))
    );

    test_target!(
        parse_target_left_and_right,
        "2-,-4",
        Ok((
            "",
            Target {
                ranges: vec![Range::Left(1), Range::Right(3)]
            }
        ))
    );

    test_target!(
        parse_target_intervals,
        "2-3,6-6,14-101",
        Ok((
            "",
            Target {
                ranges: vec![
                    Range::Interval(1, 2),
                    Range::Interval(5, 5),
                    Range::Interval(13, 100)
                ]
            }
        ))
    );

    macro_rules! test_range {
        ($name:ident, $input:expr, $want:expr) => {
            #[test]
            fn $name() {
                let got = range($input);
                assert_eq!($want, got);
            }
        };
    }

    test_range!(parse_single, "4", Ok(("", Range::Single(3))));
    test_range!(parse_left, "3-", Ok(("", Range::Left(2))));
    test_range!(parse_right, "-10", Ok(("", Range::Right(9))));
    test_range!(parse_interval, "4-8", Ok(("", Range::Interval(3, 7))));
}
