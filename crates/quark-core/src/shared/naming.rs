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
mod tests;
