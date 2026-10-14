//! Renders Arrow scalar values (numbers, dates, times, intervals, decimals, binary) as JSON.

use chrono::{DateTime, NaiveTime, Offset, TimeZone, Timelike};
use duckdb::arrow::array::{Array, ArrowPrimitiveType, AsArray, PrimitiveArray};
use duckdb::arrow::datatypes::{Decimal128Type, Decimal256Type, IntervalMonthDayNanoType};
use serde_json::{Number, Value};

use crate::shared::types::is_timetz;
use std::fmt::Write;

/// Largest integer a JSON consumer can hold exactly (2^53 - 1).
const MAX_SAFE_INTEGER: u128 = (1 << 53) - 1;

pub(super) fn value<T: ArrowPrimitiveType>(array: &dyn Array, row: usize) -> T::Native {
    array.as_primitive::<T>().value(row)
}

pub(super) fn integer_json(number: i128) -> Value {
    if number.unsigned_abs() <= MAX_SAFE_INTEGER {
        Value::from(number as i64)
    } else {
        Value::String(number.to_string())
    }
}

pub(super) fn float_json(number: f64) -> Value {
    Number::from_f64(number).map_or(Value::Null, Value::Number)
}

/// Splits `ticks` of `1 / per_second` seconds into whole seconds and microseconds.
/// Rounds toward negative infinity, so sub-microsecond ticks are dropped, never rounded.
pub(super) fn split(ticks: i64, per_second: i64) -> (i64, u32) {
    let micros = ticks.rem_euclid(per_second) * 1_000_000 / per_second;
    (ticks.div_euclid(per_second), micros as u32)
}

/// `HH:MM:SS`, then `.ffffff` only when the microseconds are non-zero.
fn write_time(text: &mut String, time: NaiveTime) {
    let _ = write!(text, "{}", time.format("%H:%M:%S"));
    let micros = time.nanosecond() / 1000;
    if micros != 0 {
        let _ = write!(text, ".{micros:06}");
    }
}

pub(super) fn date_json(days: i32) -> Value {
    DateTime::from_timestamp(i64::from(days) * 86_400, 0).map_or(Value::Null, |date| {
        Value::String(date.date_naive().to_string())
    })
}

pub(super) fn time_json((seconds, micros): (i64, u32)) -> Value {
    u32::try_from(seconds)
        .ok()
        .and_then(|seconds| NaiveTime::from_num_seconds_from_midnight_opt(seconds, micros * 1000))
        .map_or(Value::Null, |time| {
            let mut text = String::with_capacity(15);
            write_time(&mut text, time);
            Value::String(text)
        })
}

/// `zone` is `Some` for timestamps with a time zone, which render in it with a `±HH:MM` suffix.
pub(super) fn timestamp_json<Tz: TimeZone>(
    (seconds, micros): (i64, u32),
    zone: Option<&Tz>,
) -> Value {
    let Some(instant) = DateTime::from_timestamp(seconds, micros * 1000) else {
        return Value::Null;
    };
    let (naive, offset) = match zone {
        Some(zone) => {
            let local = instant.with_timezone(zone);
            (local.naive_local(), Some(local.offset().fix()))
        }
        None => (instant.naive_utc(), None),
    };
    let mut text = String::with_capacity(32);
    let _ = write!(text, "{}T", naive.date());
    write_time(&mut text, naive.time());
    if let Some(offset) = offset {
        let _ = write!(text, "{offset}");
    }
    Value::String(text)
}

/// Total seconds, counting a month as 30 days.
pub(super) fn interval_json(
    interval: <IntervalMonthDayNanoType as ArrowPrimitiveType>::Native,
) -> Value {
    let (months, days, nanos) = IntervalMonthDayNanoType::to_parts(interval);
    let micros =
        (i128::from(months) * 30 + i128::from(days)) * 86_400_000_000 + i128::from(nanos / 1000);
    float_json(micros as f64 / 1e6)
}

/// DuckDB exports HUGEINT as Decimal128(38, 0) and UHUGEINT as the same bits reinterpreted.
pub(super) fn decimal128_json(
    array: &PrimitiveArray<Decimal128Type>,
    row: usize,
    type_name: &str,
) -> Value {
    let raw = array.value(row);
    match type_name.to_ascii_uppercase().as_str() {
        "HUGEINT" => integer_json(raw),
        "UHUGEINT" => match i128::try_from(raw as u128) {
            Ok(number) => integer_json(number),
            Err(_) => Value::String((raw as u128).to_string()),
        },
        _ => decimal_float(&array.value_as_string(row)),
    }
}

pub(super) fn decimal256_json(
    array: &PrimitiveArray<Decimal256Type>,
    row: usize,
    type_name: &str,
) -> Value {
    let raw = array.value(row);
    match type_name.to_ascii_uppercase().as_str() {
        "HUGEINT" | "UHUGEINT" => raw
            .to_i128()
            .map_or_else(|| Value::String(raw.to_string()), integer_json),
        _ => decimal_float(&array.value_as_string(row)),
    }
}

/// `text` is the exact decimal rendering, so parsing it rounds correctly.
fn decimal_float(text: &str) -> Value {
    text.parse().map_or(Value::Null, float_json)
}

/// A TIMETZ arrives as text; any other string passes through.
pub(super) fn string_json(text: &str, type_name: &str) -> Value {
    if is_timetz(type_name) {
        timetz_json(text)
    } else {
        Value::String(text.to_owned())
    }
}

/// DuckDB's `HH:MM:SS[.f]±HH[:MM]` as Python's `isoformat()`: `±HH:MM`, microseconds only when set.
/// Text that does not parse (such as `24:00:00+00`) is kept as it is.
fn timetz_json(text: &str) -> Value {
    let parsed = text.rfind(['+', '-']).and_then(|split| {
        let (time, offset) = text.split_at(split);
        let time = NaiveTime::parse_from_str(time, "%H:%M:%S%.f").ok()?;
        let (hours, minutes) = match offset[1..].split_once(':') {
            Some((hours, minutes)) => (hours, minutes),
            None => (&offset[1..], "00"),
        };
        let (hours, minutes) = (hours.parse::<u8>().ok()?, minutes.parse::<u8>().ok()?);
        Some((time, &offset[..1], hours, minutes))
    });
    let Some((time, sign, hours, minutes)) = parsed else {
        return Value::String(text.to_owned());
    };
    let mut rendered = String::with_capacity(21);
    write_time(&mut rendered, time);
    let _ = write!(rendered, "{sign}{hours:02}:{minutes:02}");
    Value::String(rendered)
}

/// DuckDB exports BIT as Binary: the first byte counts the padding bits that lead the rest.
pub(super) fn binary_json(bytes: &[u8], type_name: &str) -> Value {
    let is_bit = type_name
        .get(..3)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("BIT"));
    match bytes.split_first() {
        Some((&padding, bits)) if is_bit => Value::String(
            bits.iter()
                .flat_map(|byte| (0..8).rev().map(move |shift| byte >> shift & 1))
                .skip(usize::from(padding))
                .map(|bit| if bit == 1 { '1' } else { '0' })
                .collect(),
        ),
        _ => hex_json(bytes),
    }
}

pub(super) fn hex_json(bytes: &[u8]) -> Value {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0xf)]));
    }
    Value::String(text)
}
