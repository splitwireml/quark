//! Unit tests for categorical and date profiles.

use super::*;
use crate::shared::columns::ColumnMeta;
use serde_json::json;

fn relation(
    conn: &Connection,
    setup: &str,
    column: &str,
    type_name: &str,
) -> (Relation, ColumnMeta) {
    conn.execute_batch(setup).unwrap();
    let meta = ColumnMeta {
        name: column.to_owned(),
        type_name: type_name.to_owned(),
    };
    let relation = Relation {
        sql: "(SELECT * FROM t)".to_owned(),
        params: Vec::new(),
        columns: vec![meta.clone()],
    };
    (relation, meta)
}

#[test]
fn bin_count_is_clamped_square_root() {
    let bins: Vec<u64> = [0, 1, 2, 4, 5, 100, 400, 401, 1_000_000]
        .map(bin_count)
        .into();
    assert_eq!(bins, [1, 1, 2, 2, 3, 10, 20, 20, 20]);
}

#[test]
fn null_fraction_is_zero_without_rows() {
    assert_eq!(null_fraction(0, 0), 0.0);
    assert_eq!(null_fraction(1, 4), 0.25);
}

#[test]
fn categorical_top_values_order_and_limit() {
    let conn = Connection::open_in_memory().unwrap();
    let (rel, meta) = relation(
        &conn,
        "CREATE TABLE t AS SELECT CASE WHEN i % 25 = 24 THEN NULL ELSE 'v' || lpad((i % 24)::VARCHAR, 2, '0') END AS c \
         FROM range(100) r(i) UNION ALL SELECT 'zz' FROM range(5)",
        "c",
        "VARCHAR",
    );

    let profile = categorical::profile(&conn, &rel, &meta).unwrap();

    assert_eq!(profile["kind"], "categorical");
    assert_eq!(profile["type"], "VARCHAR");
    assert_eq!(profile["row_count"], 105);
    assert_eq!(profile["null_count"], 4);
    assert_eq!(profile["non_null_count"], 101);
    assert_eq!(profile["distinct_count"], 25);
    assert_eq!(profile["null_fraction"], json!(4.0 / 105.0));
    let top = profile["top_values"].as_array().unwrap();
    assert_eq!(top.len(), 20);
    assert_eq!(top[0], json!({"value": "zz", "count": 5}));
    assert_eq!(top[1], json!({"value": "v00", "count": 4}));
    let counts: Vec<u64> = top.iter().map(|v| v["count"].as_u64().unwrap()).collect();
    assert!(counts.windows(2).all(|w| w[0] >= w[1]));
}

#[test]
fn date_bins_follow_the_square_root_rule() {
    let conn = Connection::open_in_memory().unwrap();
    let (rel, meta) = relation(
        &conn,
        "CREATE TABLE t AS SELECT DATE '2024-01-01' + i::INTEGER AS d FROM range(9) r(i) \
         UNION ALL SELECT NULL",
        "d",
        "DATE",
    );

    let profile = date::profile(&conn, &rel, &meta).unwrap();

    assert_eq!(profile["kind"], "date");
    assert_eq!(profile["row_count"], 10);
    assert_eq!(profile["null_count"], 1);
    assert_eq!(profile["null_fraction"], json!(0.1));
    assert_eq!(profile["min"], "2024-01-01");
    assert_eq!(profile["max"], "2024-01-09");
    assert_eq!(
        profile["histogram"],
        json!([
            {"lower": "2024-01-01", "upper": "2024-01-03", "count": 3},
            {"lower": "2024-01-04", "upper": "2024-01-06", "count": 3},
            {"lower": "2024-01-07", "upper": "2024-01-09", "count": 3},
        ])
    );
    assert_eq!(
        profile["year_counts"],
        json!([{"year": "2024", "count": 9}])
    );
}

#[test]
fn year_counts_only_for_dates() {
    let conn = Connection::open_in_memory().unwrap();
    for (type_name, literal) in [
        ("DATE", "DATE '2023-05-01'"),
        ("TIME", "TIME '10:00:00'"),
        ("TIMESTAMP", "TIMESTAMP '2023-05-01 10:00:00'"),
    ] {
        conn.execute_batch("DROP TABLE IF EXISTS t").unwrap();
        let setup = format!("CREATE TABLE t AS SELECT {literal} AS d");
        let (rel, meta) = relation(&conn, &setup, "d", type_name);

        let profile = date::profile(&conn, &rel, &meta).unwrap();

        let years = profile["year_counts"].as_array().unwrap();
        assert_eq!(years.len(), usize::from(type_name == "DATE"), "{type_name}");
        assert_eq!(profile["histogram"].as_array().unwrap().len(), 1);
    }
}

#[test]
fn empty_relation_profiles_to_zeros() {
    let conn = Connection::open_in_memory().unwrap();
    let (rel, meta) = relation(&conn, "CREATE TABLE t (d DATE)", "d", "DATE");

    let date = date::profile(&conn, &rel, &meta).unwrap();
    let categorical = categorical::profile(&conn, &rel, &meta).unwrap();

    assert_eq!(date["row_count"], 0);
    assert_eq!(date["null_fraction"], json!(0.0));
    assert!(date["null_fraction"].is_f64());
    assert_eq!(date["min"], Value::Null);
    assert_eq!(date["histogram"], json!([]));
    assert_eq!(date["year_counts"], json!([]));
    assert_eq!(categorical["distinct_count"], 0);
    assert_eq!(categorical["top_values"], json!([]));
}
