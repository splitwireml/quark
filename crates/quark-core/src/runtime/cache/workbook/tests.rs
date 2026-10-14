//! Tests for staging workbook sheets, one per `read_xlsx` rule in the workbook reading rules.

use super::*;
use rust_xlsxwriter::{ExcelDateTime, Format, Formula, Workbook, Worksheet};

type Cells = Vec<Vec<Option<String>>>;

fn text_row(values: &[&str]) -> Vec<Option<String>> {
    values
        .iter()
        .map(|value| Some((*value).to_owned()))
        .collect()
}

fn names(columns: &[(String, Declared)]) -> Vec<&str> {
    columns.iter().map(|(name, _)| name.as_str()).collect()
}

fn declared(columns: &[(String, Declared)]) -> Vec<Declared> {
    columns.iter().map(|(_, declared)| *declared).collect()
}

/// Writes one sheet named `sheet` with `fill`, stages it, and returns the columns and the staged text.
fn stage_named(sheet: &str, fill: impl FnOnce(&mut Worksheet)) -> (Vec<(String, Declared)>, Cells) {
    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet.set_name(sheet).unwrap();
    fill(worksheet);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("book.xlsx");
    workbook.save(&path).unwrap();
    let conn = Connection::open_in_memory().unwrap();
    let columns = stage_sheet(&conn, &path, sheet, sheet).unwrap();
    let mut statement = conn
        .prepare(&format!(
            "SELECT * FROM {} ORDER BY rowid",
            quote_ident(sheet)
        ))
        .unwrap();
    let cells = statement
        .query_map([], |row| (0..columns.len()).map(|at| row.get(at)).collect())
        .unwrap()
        .collect::<Result<Cells, _>>()
        .unwrap();
    (columns, cells)
}

fn stage(fill: impl FnOnce(&mut Worksheet)) -> (Vec<(String, Declared)>, Cells) {
    stage_named("Sheet", fill)
}

fn write_strings(sheet: &mut Worksheet, row: u32, values: &[&str]) {
    for (column, value) in (0u16..).zip(values) {
        sheet.write_string(row, column, *value).unwrap();
    }
}

fn write_numbers(sheet: &mut Worksheet, row: u32, values: &[f64]) {
    for (column, value) in (0u16..).zip(values) {
        sheet.write_number(row, column, *value).unwrap();
    }
}

#[test]
fn integers_and_floats_stage_as_shortest_text() {
    let (columns, cells) = stage(|sheet| {
        write_strings(sheet, 0, &["whole", "fraction", "mixed"]);
        write_numbers(sheet, 1, &[1.0, 0.5, 1.0]);
        write_numbers(sheet, 2, &[2.0, 1.25, 2.5]);
        write_numbers(sheet, 3, &[-3.0, 1e6, 3.0]);
    });
    assert_eq!(names(&columns), ["whole", "fraction", "mixed"]);
    assert_eq!(declared(&columns), [Declared::Other; 3]);
    assert_eq!(
        cells,
        [
            text_row(&["1", "0.5", "1"]),
            text_row(&["2", "1.25", "2.5"]),
            text_row(&["-3", "1000000", "3"])
        ]
    );
}

#[test]
fn floats_beyond_sixteen_digits_use_exponent_text() {
    let (_, cells) = stage(|sheet| {
        write_strings(sheet, 0, &["v"]);
        let values = [
            0.1 + 0.2,
            1.0 / 3.0,
            12345678901234567890.0,
            1e-7,
            2f64.powi(60),
            123_456_789.123_456_79,
        ];
        for (row, value) in (1u32..).zip(values) {
            sheet.write_number(row, 0, value).unwrap();
        }
    });
    let texts: Vec<_> = cells.iter().map(|row| row[0].clone().unwrap()).collect();
    assert_eq!(
        texts,
        [
            "0.3",
            "0.3333333333333333",
            "1.234567890123457e+19",
            "1e-07",
            "1.152921504606847e+18",
            "123456789.1234568"
        ]
    );
}

#[test]
fn dates_stage_as_excel_serials() {
    let day = Format::new().set_num_format("yyyy-mm-dd");
    let moment = Format::new().set_num_format("yyyy-mm-dd hh:mm:ss");
    let clock = Format::new().set_num_format("hh:mm:ss");
    let (columns, cells) = stage(|sheet| {
        write_strings(sheet, 0, &["day", "moment", "clock"]);
        let rows = [
            ("2024-01-02", "2024-01-02 12:00:00", "09:30:15"),
            ("2025-12-31", "2025-12-31 23:59:59", "00:00:00"),
        ];
        for (row, (d, m, c)) in (1u32..).zip(rows) {
            sheet
                .write_datetime_with_format(row, 0, ExcelDateTime::parse_from_str(d).unwrap(), &day)
                .unwrap();
            sheet
                .write_datetime_with_format(
                    row,
                    1,
                    ExcelDateTime::parse_from_str(m).unwrap(),
                    &moment,
                )
                .unwrap();
            sheet
                .write_datetime_with_format(
                    row,
                    2,
                    ExcelDateTime::parse_from_str(c).unwrap(),
                    &clock,
                )
                .unwrap();
        }
    });
    assert_eq!(
        declared(&columns),
        [Declared::Date, Declared::Timestamp, Declared::Time]
    );
    assert_eq!(
        cells,
        [
            text_row(&["45293", "45293.5", "0.3960069444444445"]),
            text_row(&["46022", "46022.99998842592", "0"])
        ]
    );
}

#[test]
fn declared_type_comes_from_the_first_data_row() {
    let day = Format::new().set_num_format("yyyy-mm-dd");
    let date = ExcelDateTime::parse_from_str("2024-01-02").unwrap();
    let (columns, cells) = stage(|sheet| {
        write_strings(
            sheet,
            0,
            &["date_text", "num_date", "gap_date", "text_date"],
        );
        sheet.write_datetime_with_format(1, 0, &date, &day).unwrap();
        sheet.write_number(1, 1, 5).unwrap();
        sheet.write_string(1, 3, "n/a").unwrap();
        sheet.write_string(2, 0, "n/a").unwrap();
        for column in 1..4 {
            sheet
                .write_datetime_with_format(2, column, &date, &day)
                .unwrap();
        }
    });
    assert_eq!(
        names(&columns),
        ["date_text", "num_date", "gap_date", "text_date"]
    );
    let other = Declared::Other;
    assert_eq!(declared(&columns), [Declared::Date, other, other, other]);
    let some = |text: &str| Some(text.to_owned());
    assert_eq!(
        cells,
        [
            vec![some("45293"), some("5"), None, some("n/a")],
            vec![some("n/a"), some("45293"), some("45293"), some("45293")]
        ]
    );
}

#[test]
fn booleans_stage_as_one_and_zero() {
    let (columns, cells) = stage(|sheet| {
        write_strings(sheet, 0, &["flag", "maybe"]);
        sheet.write_boolean(1, 0, true).unwrap();
        sheet.write_boolean(1, 1, true).unwrap();
        sheet.write_boolean(2, 0, false).unwrap();
        sheet.write_boolean(3, 0, true).unwrap();
        sheet.write_boolean(3, 1, false).unwrap();
    });
    assert_eq!(declared(&columns), [Declared::Other; 2]);
    assert_eq!(
        cells,
        [
            text_row(&["1", "1"]),
            vec![Some("0".to_owned()), None],
            text_row(&["1", "0"])
        ]
    );
}

#[test]
fn text_identifiers_keep_their_leading_zeros() {
    let (columns, cells) = stage(|sheet| {
        write_strings(sheet, 0, &["code", "sku", "name"]);
        write_strings(sheet, 1, &["007", "00-1", "Bond"]);
        write_strings(sheet, 2, &["1e3", "0x10", "Twelve"]);
    });
    assert_eq!(names(&columns), ["code", "sku", "name"]);
    assert_eq!(declared(&columns), [Declared::Other; 3]);
    assert_eq!(
        cells,
        [
            text_row(&["007", "00-1", "Bond"]),
            text_row(&["1e3", "0x10", "Twelve"])
        ]
    );
}

#[test]
fn empty_cells_are_null_and_an_empty_row_ends_the_read() {
    let (columns, cells) = stage(|sheet| {
        write_strings(sheet, 0, &["a", "b", "c"]);
        sheet.write_number(1, 0, 1).unwrap();
        sheet.write_string(1, 2, "x").unwrap();
        sheet.write_number(3, 0, 3).unwrap();
        sheet.write_number(3, 1, 4.5).unwrap();
    });
    assert_eq!(names(&columns), ["a", "b", "c"]);
    assert_eq!(declared(&columns), [Declared::Other; 3]);
    assert_eq!(
        cells,
        [vec![Some("1".to_owned()), None, Some("x".to_owned())]]
    );
}

#[test]
fn numeric_first_row_means_no_header() {
    let (columns, cells) = stage(|sheet| {
        write_numbers(sheet, 0, &[1.0, 2.0, 3.0]);
        write_numbers(sheet, 1, &[4.0, 5.0, 6.0]);
    });
    assert_eq!(names(&columns), ["A1", "B1", "C1"]);
    assert_eq!(declared(&columns), [Declared::Other; 3]);
    assert_eq!(
        cells,
        [text_row(&["1", "2", "3"]), text_row(&["4", "5", "6"])]
    );
}

#[test]
fn a_gap_in_the_first_row_means_no_header_and_keeps_the_gap_column() {
    let (columns, cells) = stage(|sheet| {
        sheet.write_string(1, 1, "a").unwrap();
        sheet.write_string(1, 3, "c").unwrap();
        for (column, value) in (1u16..).zip(["x", "y", "z"]) {
            sheet.write_string(2, column, value).unwrap();
        }
    });
    assert_eq!(names(&columns), ["B2", "C2", "D2"]);
    assert_eq!(declared(&columns), [Declared::Other; 3]);
    assert_eq!(
        cells,
        [
            vec![Some("a".to_owned()), None, Some("c".to_owned())],
            text_row(&["x", "y", "z"])
        ]
    );
}

#[test]
fn header_only_sheet_has_no_rows_and_keeps_its_names() {
    let (columns, cells) = stage(|sheet| write_strings(sheet, 0, &["alpha", "beta"]));
    assert_eq!(names(&columns), ["alpha", "beta"]);
    assert_eq!(declared(&columns), [Declared::Other; 2]);
    assert!(cells.is_empty());
}

#[test]
fn duplicate_header_names_get_their_column_position() {
    let (columns, cells) = stage(|sheet| {
        write_strings(sheet, 0, &["name", "name", "Name", "x"]);
        write_strings(sheet, 1, &["a", "b", "c", "d"]);
    });
    assert_eq!(names(&columns), ["name", "name_1", "Name_2", "x"]);
    assert_eq!(declared(&columns), [Declared::Other; 4]);
    assert_eq!(cells, [text_row(&["a", "b", "c", "d"])]);
}

#[test]
fn sheet_names_with_apostrophes_and_accents_stage_and_list_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("book.xlsx");
    let mut workbook = Workbook::new();
    for (name, header, value) in [
        ("O'Reilly", "book", "Learning SQL"),
        ("Café Müller", "city", "Zürich"),
    ] {
        let sheet = workbook.add_worksheet();
        sheet.set_name(name).unwrap();
        write_strings(sheet, 0, &[header]);
        write_strings(sheet, 1, &[value]);
    }
    workbook.save(&path).unwrap();
    assert_eq!(sheet_names(&path).unwrap(), ["O'Reilly", "Café Müller"]);
    let conn = Connection::open_in_memory().unwrap();
    stage_sheet(&conn, &path, "Café Müller", "Café Müller").unwrap();
    let city: String = conn
        .query_row("SELECT city FROM \"Café Müller\"", [], |row| row.get(0))
        .unwrap();
    assert_eq!(city, "Zürich");
}

#[test]
fn fifty_columns_stage_in_order() {
    let header: Vec<String> = (1..=50).map(|at| format!("col{at}")).collect();
    let (columns, cells) = stage(|sheet| {
        for (column, name) in (0u16..).zip(&header) {
            sheet.write_string(0, column, name).unwrap();
            sheet
                .write_number(1, column, f64::from(column) + 1.0)
                .unwrap();
        }
    });
    assert_eq!(columns.len(), 50);
    assert_eq!(columns[49].0, "col50");
    assert_eq!(cells[0][49].as_deref(), Some("50"));
}

#[test]
fn formula_like_text_is_never_evaluated_or_coerced() {
    let (columns, cells) = stage(|sheet| {
        write_strings(sheet, 0, &["text", "other"]);
        write_strings(sheet, 1, &["=1+1", "plain"]);
        write_strings(sheet, 2, &["+SUM(A1)", "@cmd"]);
        write_strings(sheet, 3, &["-5x", "ok"]);
    });
    assert_eq!(names(&columns), ["text", "other"]);
    assert_eq!(declared(&columns), [Declared::Other; 2]);
    assert_eq!(
        cells,
        [
            text_row(&["=1+1", "plain"]),
            text_row(&["+SUM(A1)", "@cmd"]),
            text_row(&["-5x", "ok"])
        ]
    );
}

#[test]
fn cached_formula_results_stage_as_values() {
    let (columns, cells) = stage(|sheet| {
        write_strings(sheet, 0, &["total"]);
        sheet
            .write_formula(1, 0, Formula::new("=1+1").set_result("2"))
            .unwrap();
    });
    assert_eq!(names(&columns), ["total"]);
    assert_eq!(declared(&columns), [Declared::Other]);
    assert_eq!(cells, [text_row(&["2"])]);
}

#[test]
fn a_hundred_thousand_rows_stage_in_one_pass() {
    let (columns, cells) = stage(|sheet| {
        write_strings(sheet, 0, &["n", "label"]);
        for row in 1..=100_000u32 {
            sheet.write_number(row, 0, row).unwrap();
            sheet.write_string(row, 1, "row").unwrap();
        }
    });
    assert_eq!(columns.len(), 2);
    assert_eq!(cells.len(), 100_000);
    assert_eq!(cells[99_999][0].as_deref(), Some("100000"));
}
