//! The text forms of workbook cells and column letters, as `read_xlsx` with `all_varchar` writes them.

use calamine::DataRef;

/// Spreadsheet column letters for a 0-based index: 0 is `A`, 26 is `AA`.
pub(super) fn column_letters(index: u32) -> String {
    let letter = char::from(b'A' + (index % 26) as u8);
    match index {
        0..26 => letter.to_string(),
        _ => format!("{}{letter}", column_letters(index / 26 - 1)),
    }
}

/// The text `read_xlsx(all_varchar = true)` gives a cell; `None` for an empty cell.
pub(super) fn cell_text(cell: &DataRef<'_>) -> Option<String> {
    let text = match cell {
        DataRef::Empty => return None,
        DataRef::String(text) | DataRef::DateTimeIso(text) | DataRef::DurationIso(text) => {
            text.clone()
        }
        DataRef::SharedString(text) => (*text).to_owned(),
        DataRef::Int(whole) => whole.to_string(),
        DataRef::Float(number) => number_text(*number),
        DataRef::Bool(flag) => String::from(if *flag { "1" } else { "0" }),
        DataRef::DateTime(stamp) => number_text(stamp.as_f64()),
        DataRef::Error(error) => error.to_string(),
    };
    Some(text).filter(|text| !text.is_empty())
}

/// A float the way C's `%.16g` writes it: 16 significant digits, no trailing zeros, exponent form
/// below 1e-4 and from 1e16.
fn number_text(value: f64) -> String {
    let scientific = format!("{value:.15e}");
    let Some((mantissa, exponent)) = scientific
        .split_once('e')
        .and_then(|(mantissa, exponent)| Some((mantissa, exponent.parse::<i32>().ok()?)))
    else {
        return scientific;
    };
    if (-4..16).contains(&exponent) {
        let fixed = format!("{value:.places$}", places = (15 - exponent) as usize);
        return trim_zeros(&fixed).to_owned();
    }
    let sign = if exponent < 0 { '-' } else { '+' };
    format!("{}e{sign}{:02}", trim_zeros(mantissa), exponent.abs())
}

fn trim_zeros(text: &str) -> &str {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        text
    }
}
