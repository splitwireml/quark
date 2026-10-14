//! Tests for export request bounds and export naming.

use std::collections::HashSet;

use super::*;
use crate::features::exports::model::{ExportRequest, ExportSheet, JsonLayout};

fn names(inputs: &[&str]) -> Vec<String> {
    let mut used = HashSet::new();
    inputs
        .iter()
        .map(|n| export_sheet_name(n, &mut used))
        .collect()
}

#[test]
fn sheet_names_are_sanitized_truncated_and_unique() {
    assert_eq!(names(&["a/b\\c*d?e:f[g]h"]), ["a_b_c_d_e_f_g_h"]);
    assert_eq!(names(&["  'quoted'  "]), ["quoted"]);
    assert_eq!(
        names(&["", "'", "\u{1}\u{b}"]),
        ["Sheet", "Sheet (2)", "Sheet (3)"]
    );
    assert_eq!(names(&["a\u{0}b\u{ffff}c"]), ["abc"]);
    assert_eq!(names(&["x".repeat(40).as_str()]), ["x".repeat(31)]);
    assert_eq!(
        names(&[&format!("{}'tail", "y".repeat(30))]),
        ["y".repeat(30)]
    );
    assert_eq!(
        names(&["Data", "data", "DATA"]),
        ["Data", "data (2)", "DATA (3)"]
    );
    let long = "z".repeat(31);
    assert_eq!(
        names(&[&long, &long]),
        [long.clone(), format!("{} (2)", "z".repeat(27))]
    );
    let apostrophe = format!("{}'{}", "q".repeat(26), "r".repeat(10));
    assert_eq!(
        names(&[&apostrophe, &apostrophe]),
        [
            format!("{}'{}", "q".repeat(26), "r".repeat(4)),
            format!("{} (2)", "q".repeat(26))
        ]
    );
    assert_eq!(names(&["日本語"]), ["日本語"]);
}

#[test]
fn history_is_renamed() {
    assert_eq!(
        names(&["History", "HISTORY", "history_"]),
        ["History_", "HISTORY_ (2)", "history_ (3)"]
    );
}

#[test]
fn filename_keeps_only_a_safe_stem() {
    assert_eq!(export_filename(None, ".csv"), "export.csv");
    assert_eq!(export_filename(Some(""), ".csv"), "export.csv");
    assert_eq!(export_filename(Some("report.csv"), ".xlsx"), "report.xlsx");
    assert_eq!(export_filename(Some("a.tar.gz"), ".json"), "a.tar.json");
    assert_eq!(
        export_filename(Some("..\\..\\etc/passwd"), ".csv"),
        "passwd.csv"
    );
    assert_eq!(export_filename(Some("dir/sub/"), ".csv"), "sub.csv");
    assert_eq!(export_filename(Some("/"), ".csv"), "export.csv");
    assert_eq!(export_filename(Some(".."), ".csv"), "export.csv");
    assert_eq!(export_filename(Some(".bashrc"), ".csv"), "bashrc.csv");
    assert_eq!(export_filename(Some("name."), ".csv"), "name.csv");
    assert_eq!(
        export_filename(Some(" a\u{0}b\u{7f}c\n. "), ".csv"),
        "abc.csv"
    );
    assert_eq!(export_filename(Some("日本.語"), ".csv"), "日本.csv");
}

#[test]
fn content_disposition_quotes_non_ascii_names() {
    assert_eq!(
        content_disposition("report-1_a.csv"),
        "attachment; filename=\"report-1_a.csv\""
    );
    assert_eq!(
        content_disposition("my report.csv"),
        "attachment; filename*=utf-8''my%20report.csv"
    );
    assert_eq!(
        content_disposition("café.csv"),
        "attachment; filename*=utf-8''caf%C3%A9.csv"
    );
    assert_eq!(
        content_disposition("a\"b.csv"),
        "attachment; filename*=utf-8''a%22b.csv"
    );
}

#[test]
fn formats_map_to_media_types_and_extensions() {
    assert_eq!(media_type(ExportFormat::Csv), "text/csv");
    assert_eq!(
        media_type(ExportFormat::Xlsx),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );
    assert_eq!(
        media_type(ExportFormat::Parquet),
        "application/vnd.apache.parquet"
    );
    assert_eq!(media_type(ExportFormat::Json), "application/json");
    assert_eq!(extension(ExportFormat::Xlsx), ".xlsx");
    assert_eq!(extension(ExportFormat::Parquet), ".parquet");
}

fn request(sheets: usize) -> ExportRequest {
    let sheet = ExportSheet {
        node_id: "n".into(),
        name: "s".into(),
        sql: "select 1".into(),
    };
    ExportRequest {
        format: ExportFormat::Csv,
        json_layout: JsonLayout::Rows,
        filename: None,
        sheets: vec![sheet; sheets],
    }
}

#[test]
fn sheet_count_bounds() {
    assert_eq!(request(0).validate().unwrap_err().status().as_u16(), 422);
    assert!(request(1).validate().is_ok());
    assert!(request(100).validate().is_ok());
    assert_eq!(request(101).validate().unwrap_err().status().as_u16(), 422);
}

#[test]
fn request_defaults_and_unknown_fields() {
    let sheet = r#"{"node_id":"n","name":"s","sql":"select 1"}"#;
    let ok = format!(r#"{{"format":"json","sheets":[{sheet}]}}"#);
    let parsed: ExportRequest = serde_json::from_str(&ok).unwrap();
    assert_eq!(parsed.json_layout, JsonLayout::Rows);
    assert_eq!(parsed.filename, None);
    let layout = format!(r#"{{"format":"json","json_layout":"columns","sheets":[{sheet}]}}"#);
    let parsed: ExportRequest = serde_json::from_str(&layout).unwrap();
    assert_eq!(parsed.json_layout, JsonLayout::Columns);
    let extra = format!(r#"{{"format":"csv","sheets":[{sheet}],"extra":1}}"#);
    assert!(serde_json::from_str::<ExportRequest>(&extra).is_err());
    let bad = format!(r#"{{"format":"pdf","sheets":[{sheet}]}}"#);
    assert!(serde_json::from_str::<ExportRequest>(&bad).is_err());
}
