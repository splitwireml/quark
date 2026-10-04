//! Dataset naming and page counting, ported from the Python backend.

/// Derives a SQL-safe dataset name from an uploaded file name.
pub fn dataset_name(filename: &str) -> String {
    let base = filename.rsplit('/').next().unwrap_or(filename);
    // Like Python's `Path.stem`: drop the last extension, but keep a leading dot.
    let stem = match base.rfind('.') {
        Some(dot) if dot > 0 => &base[..dot],
        _ => base,
    };
    let mut name = String::with_capacity(stem.len());
    for ch in stem.to_lowercase().chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            name.push(ch);
        } else if !name.ends_with('_') {
            name.push('_');
        }
    }
    let name = name.trim_matches('_');
    match name.chars().next() {
        None => "data".to_owned(),
        Some(first) if first.is_ascii_digit() => format!("data_{name}"),
        Some(_) => name.to_owned(),
    }
}

/// Number of pages needed for `rows` rows; `page_size` must be at least 1.
pub fn page_count(rows: u64, page_size: u64) -> u64 {
    rows.div_ceil(page_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_name_matches_python() {
        let cases = [
            ("Claims v1.csv", "claims_v1"),
            ("2024 sales.csv", "data_2024_sales"),
            ("___.csv", "data"),
            (".csv", "csv"),
            ("Zoë Data.parquet", "zo_data"),
            ("x.tsv", "x"),
            ("ÅÄÖ.json", "data"),
        ];
        for (file_name, expected) in cases {
            assert_eq!(dataset_name(file_name), expected, "{file_name}");
        }
    }

    #[test]
    fn page_count_rounds_up() {
        let cases = [
            (0, 100, 0),
            (1, 100, 1),
            (100, 100, 1),
            (101, 100, 2),
            (9_007_199_254_740_993, 1, 9_007_199_254_740_993),
        ];
        for (rows, page_size, expected) in cases {
            assert_eq!(page_count(rows, page_size), expected, "{rows}/{page_size}");
        }
    }
}
