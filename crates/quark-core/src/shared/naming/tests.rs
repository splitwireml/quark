//! Unit tests for dataset names and page counts.

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
