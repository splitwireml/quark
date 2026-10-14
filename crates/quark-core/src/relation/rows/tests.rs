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

fn column(name: &str, type_name: &str) -> ColumnMeta {
    ColumnMeta {
        name: name.to_owned(),
        type_name: type_name.to_owned(),
    }
}

#[test]
fn select_list_is_star_without_timetz() {
    let columns = [column("a", "INTEGER"), column("b", "TIME")];

    assert_eq!(select_list(&columns), "*");
    assert_eq!(select_list(&[]), "*");
}

#[test]
fn select_list_casts_only_timetz_columns() {
    let columns = [
        column("a", "INTEGER"),
        column("t\"z", "TIME WITH TIME ZONE"),
        column("u", "time with time zone"),
    ];

    assert_eq!(
        select_list(&columns),
        "* REPLACE (CAST(\"t\"\"z\" AS VARCHAR) AS \"t\"\"z\", CAST(\"u\" AS VARCHAR) AS \"u\")"
    );
}

#[test]
fn json_rows_render_timetz_with_its_offset() {
    let conn = Connection::open_in_memory().unwrap();
    let sql = "SELECT TIMETZ '12:30:00-02' AS a, TIMETZ '01:02:03.5+05:30' AS b";

    let (columns, rows) = json_rows(&conn, sql, &[]).unwrap();

    assert_eq!(columns[0].type_name, "TIME WITH TIME ZONE");
    assert_eq!(
        rows,
        vec![vec![
            json!("12:30:00-02:00"),
            json!("01:02:03.500000+05:30")
        ]]
    );
}
