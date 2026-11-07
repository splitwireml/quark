//! SQL literal rendering that matches the Python backend.

use serde_json::Value;

use crate::shared::sql::sql_string;

/// Formats `x` the way Python's `repr(float)` does.
pub fn python_float_repr(x: f64) -> String {
    let scientific = format!("{x:e}");
    let Some((mantissa, exponent)) = scientific.split_once('e') else {
        return scientific;
    };
    let Ok(exponent) = exponent.parse::<i32>() else {
        return scientific;
    };
    let (sign, mantissa) = match mantissa.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", mantissa),
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();

    if !(-4..16).contains(&exponent) {
        let (head, tail) = digits.split_at(1);
        let point = if tail.is_empty() { "" } else { "." };
        let exponent_sign = if exponent < 0 { '-' } else { '+' };
        return format!(
            "{sign}{head}{point}{tail}e{exponent_sign}{:02}",
            exponent.abs()
        );
    }
    match usize::try_from(exponent) {
        Ok(point) if digits.len() > point + 1 => {
            let (whole, fraction) = digits.split_at(point + 1);
            format!("{sign}{whole}.{fraction}")
        }
        Ok(point) => format!("{sign}{digits}{}.0", "0".repeat(point + 1 - digits.len())),
        Err(_) => format!(
            "{sign}0.{}{digits}",
            "0".repeat(exponent.unsigned_abs() as usize - 1)
        ),
    }
}

/// Renders a JSON value as a SQL literal.
pub fn literal(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_owned(),
        Value::Bool(true) => "TRUE".to_owned(),
        Value::Bool(false) => "FALSE".to_owned(),
        Value::Number(number) => match number.as_f64() {
            Some(float) if number.is_f64() => python_float_repr(float),
            _ => number.to_string(),
        },
        Value::String(text) => sql_string(text),
        Value::Array(_) | Value::Object(_) => sql_string(&value.to_string()),
    }
}
#[cfg(test)]
mod tests;
