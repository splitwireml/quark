//! Unit tests for JSON row conversion.

use super::*;
use serde_json::json;

#[test]
fn json_rows_follow_safe_rules() {
    let conn = Connection::open_in_memory().unwrap();
    let sql = "SELECT 18446744073709551616::HUGEINT AS big, 'NaN'::DOUBLE AS nan, \
               DATE '2024-03-05' AS day, ? AS bound";

    let (columns, rows) = json_rows(&conn, sql, &[json!("p")]).unwrap();

    let types: Vec<&str> = columns.iter().map(|c| c.type_name.as_str()).collect();
    assert_eq!(types, ["HUGEINT", "DOUBLE", "DATE", "VARCHAR"]);
    assert_eq!(
        rows,
        vec![vec![
            json!("18446744073709551616"),
            Value::Null,
            json!("2024-03-05"),
            json!("p"),
        ]]
    );
}
