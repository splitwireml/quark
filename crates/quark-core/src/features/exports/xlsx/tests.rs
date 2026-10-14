//! Tests for XLSX exports: sheet names and order, text that stays text, typed cells, the cell size limit and empty results.

use std::io::Cursor;

use axum::body::Body;
use axum::http::Response;
use calamine::{Data, Reader, Xlsx};
use chrono::NaiveDate;
use http_body_util::BodyExt;
use rust_xlsxwriter::Format;
use serde_json::{Value, json};

use super::super::tests::{app, export, exports_in, failure, sheet};
use super::{Formats, date_serial, date_time_serial};

async fn workbook(response: Response<Body>) -> Xlsx<Cursor<Vec<u8>>> {
    assert_eq!(response.status(), 200);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    Xlsx::new(Cursor::new(bytes.to_vec())).unwrap()
}

fn xlsx_request(sheets: &[Value]) -> Value {
    json!({"format": "xlsx", "sheets": sheets})
}

fn named(name: &str, sql: &str) -> Value {
    json!({"node_id": "legacy", "name": name, "sql": sql})
}

fn rows(book: &mut Xlsx<Cursor<Vec<u8>>>, name: &str) -> Vec<Vec<Data>> {
    let range = book.worksheet_range(name).unwrap();
    range.rows().map(<[Data]>::to_vec).collect()
}

#[tokio::test]
async fn sheets_follow_request_order_and_names() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let request = xlsx_request(&[
        named("Totals", "SELECT count(*) AS n FROM data"),
        named("Bad/Name*", "SELECT b FROM data WHERE a < 2 ORDER BY a"),
        named("bad_name_", "SELECT 7 AS seven"),
    ]);

    let mut book = workbook(export(&app, request).await).await;

    assert_eq!(book.sheet_names(), ["Totals", "Bad_Name_", "bad_name_ (2)"]);
    assert_eq!(rows(&mut book, "Totals")[1], [Data::Float(6000.0)]);
    let second = rows(&mut book, "Bad_Name_");
    assert_eq!(second[0], [Data::String("b".to_owned())]);
    assert_eq!(second[2], [Data::String("value-1".to_owned())]);
    assert_eq!(rows(&mut book, "bad_name_ (2)")[1], [Data::Float(7.0)]);
    assert!(exports_in(root.path()).is_empty());
}

#[tokio::test]
async fn text_is_never_a_formula() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let sql = "SELECT * FROM (VALUES ('=1+1', '@SUM(1,1)', '#N/A')) v(a, b, c)";

    let mut book = workbook(export(&app, xlsx_request(&[named("Edge", sql)])).await).await;

    let expected = ["=1+1", "@SUM(1,1)", "#N/A"].map(|text| Data::String(text.to_owned()));
    assert_eq!(rows(&mut book, "Edge")[1], expected);
    let formulas = book.worksheet_formula("Edge").unwrap();
    assert!(formulas.rows().flatten().all(String::is_empty));
}

#[tokio::test]
async fn typed_cells_keep_numbers_dates_durations_and_empty_cells() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let sql = "SELECT 3 AS i, 1.5 AS f, 12.34::DECIMAL(10,2) AS d, true AS b, NULL AS n, \
               DATE '2026-09-01' AS day, TIMESTAMP '2026-09-01 12:30:45' AS moment, \
               INTERVAL '1 day 2 hours' AS span";

    let mut book = workbook(export(&app, xlsx_request(&[named("Types", sql)])).await).await;

    let row = &rows(&mut book, "Types")[1];
    assert_eq!(
        row[..4],
        [
            Data::Float(3.0),
            Data::Float(1.5),
            Data::Float(12.34),
            Data::Bool(true)
        ]
    );
    assert_eq!(row[4], Data::Empty);
    let serial = |data: &Data| match data {
        Data::DateTime(value) => (value.as_f64(), value.is_duration()),
        other => panic!("not a date cell: {other:?}"),
    };
    assert_eq!(serial(&row[5]), (46266.0, false));
    let (moment, is_duration) = serial(&row[6]);
    assert!((moment - 46_266.521_354_166_67).abs() < 1e-9 && !is_duration);
    assert_eq!(serial(&row[7]), (26.0 / 24.0, true));
}

#[test]
fn duration_cells_use_the_openpyxl_timedelta_format() {
    // `openpyxl.styles.numbers.FORMAT_DATE_TIMEDELTA` is "[hh]:mm:ss". calamine reads "[h]:mm:ss"
    // and "[hh]:mm:ss" alike as a duration, so the string is pinned on the format the writer applies.
    assert_eq!(
        Formats::new().duration,
        Format::new().set_num_format("[hh]:mm:ss")
    );
}

#[tokio::test]
async fn oversized_text_is_422_and_leaves_no_file() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let long = |count: usize| named("Long", &format!("SELECT repeat('x', {count}) AS value"));

    let failed = export(&app, xlsx_request(&[long(32_768)])).await;

    assert_eq!(
        failure(failed).await,
        (422, "Excel cell text exceeds 32767 characters".to_owned())
    );
    assert!(exports_in(root.path()).is_empty());
    let mut book = workbook(export(&app, xlsx_request(&[long(32_767)])).await).await;
    assert_eq!(
        rows(&mut book, "Long")[1][0],
        Data::String("x".repeat(32_767))
    );
}

#[tokio::test]
async fn empty_results_export_valid_files() {
    let root = tempfile::tempdir().unwrap();
    let app = app(root.path());
    let request = xlsx_request(&[sheet("legacy", "SELECT a, b FROM data WHERE false")]);

    let mut book = workbook(export(&app, request).await).await;

    let expected = ["a", "b"].map(|name| Data::String(name.to_owned()));
    assert_eq!(rows(&mut book, "Sheet"), [expected]);
}

#[test]
fn dates_before_march_1900_follow_openpyxl_serials() {
    let date = |year, month, day| NaiveDate::from_ymd_opt(year, month, day).unwrap();

    assert_eq!(date_serial(date(1899, 12, 31)), 0.0);
    assert_eq!(date_serial(date(1900, 1, 1)), 1.0);
    assert_eq!(date_serial(date(1900, 2, 28)), 59.0);
    assert_eq!(date_serial(date(1900, 3, 1)), 61.0);
    let noon = date(1900, 1, 1).and_hms_opt(12, 0, 0).unwrap();
    assert_eq!(date_time_serial(noon), 1.5);
}
