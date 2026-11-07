//! Renders export cells as the text Python's `csv` module writes.

use super::ExportCell;
use crate::shared::literal::python_float_repr;
use chrono::{NaiveDateTime, NaiveTime, TimeDelta};
use std::fmt::Write;

const FORMULA_PREFIXES: [char; 6] = ['=', '+', '-', '@', '\t', '\r'];

/// `YYYY-MM-DD<sep>HH:MM:SS[.ffffff]`.
pub(super) fn iso_text(moment: &NaiveDateTime, separator: char) -> String {
    let mut text = format!("{}{separator}", moment.date());
    write_time(&mut text, moment.time());
    text
}

/// `HH:MM:SS`, then `.ffffff` only when the microseconds are non-zero.
fn write_time(text: &mut String, time: NaiveTime) {
    let _ = write!(text, "{}", time.format("%H:%M:%S"));
    let micros = chrono::Timelike::nanosecond(&time) / 1000;
    if micros != 0 {
        let _ = write!(text, ".{micros:06}");
    }
}

/// Python's `str(timedelta)`, such as `1 day, 2:00:00` or `-1 day, 23:59:59`.
fn timedelta_text(delta: TimeDelta) -> String {
    const DAY: i64 = 86_400_000_000;
    let micros = delta.num_microseconds().unwrap_or_default();
    let (days, rest) = (micros.div_euclid(DAY), micros.rem_euclid(DAY));
    let mut text = match days {
        0 => String::new(),
        1 | -1 => format!("{days} day, "),
        _ => format!("{days} days, "),
    };
    let _ = write!(
        text,
        "{}:{:02}:{:02}",
        rest / 3_600_000_000,
        rest / 60_000_000 % 60,
        rest / 1_000_000 % 60
    );
    if rest % 1_000_000 != 0 {
        let _ = write!(text, ".{:06}", rest % 1_000_000);
    }
    text
}

/// The field Python's `csv` module writes for `csv_value(cell)`; `None` is an empty field.
pub fn csv_text(cell: &ExportCell) -> Option<String> {
    Some(match cell {
        ExportCell::Null => return None,
        ExportCell::Bool(true) => "True".to_owned(),
        ExportCell::Bool(false) => "False".to_owned(),
        ExportCell::Int(number) => number.to_string(),
        ExportCell::Float(number) => python_float_repr(*number),
        ExportCell::Decimal(text) => text.clone(),
        ExportCell::Text(text) if text.starts_with(FORMULA_PREFIXES) => format!("'{text}"),
        ExportCell::Text(text) => text.clone(),
        ExportCell::Date(date) => date.to_string(),
        ExportCell::DateTime(moment) => iso_text(moment, ' '),
        ExportCell::Time(time) => {
            let mut text = String::with_capacity(15);
            write_time(&mut text, *time);
            text
        }
        ExportCell::Duration(delta) => timedelta_text(*delta),
    })
}

/// One row in the `excel` dialect, ending in `\r\n`.
pub fn csv_line(fields: &[Option<String>]) -> String {
    if let [only] = fields
        && only.as_deref().is_none_or(str::is_empty)
    {
        return "\"\"\r\n".to_owned();
    }
    let mut line = String::new();
    for (position, field) in fields.iter().enumerate() {
        if position > 0 {
            line.push(',');
        }
        let text = field.as_deref().unwrap_or_default();
        if text.contains([',', '"', '\r', '\n']) {
            line.push('"');
            line.push_str(&text.replace('"', "\"\""));
            line.push('"');
        } else {
            line.push_str(text);
        }
    }
    line.push_str("\r\n");
    line
}
