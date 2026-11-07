//! Writes one sheet as CSV: a header row, then every row through the export cell rules.

use std::io::Write;

use duckdb::Connection;

use super::service::{PreparedSheet, write_failed};
use super::stream::for_each_batch;
use crate::error::ApiResult;
use crate::relation::rows::select_list;
use crate::shared::encode::export::{ExportCell, csv_line, csv_text, export_cell, is_illegal_xml};

/// Streams the sheet's rows into `output` as the Python `csv` writer would, and flushes it.
pub(super) fn write_csv(
    conn: &Connection,
    sheet: &PreparedSheet,
    output: &mut impl Write,
) -> ApiResult<()> {
    let header: Vec<Option<String>> = sheet
        .header
        .iter()
        .map(|name| {
            let name = name.chars().filter(|c| !is_illegal_xml(*c)).collect();
            csv_text(&ExportCell::Text(name))
        })
        .collect();
    output
        .write_all(csv_line(&header).as_bytes())
        .map_err(write_failed)?;

    let select = format!(
        "SELECT {} FROM ({})",
        select_list(&sheet.columns),
        sheet.sql
    );
    for_each_batch(conn, &select, |batch| {
        for row in 0..batch.num_rows() {
            let fields: Vec<Option<String>> = sheet
                .columns
                .iter()
                .zip(batch.columns())
                .map(|(meta, array)| {
                    let cell = export_cell(array.as_ref(), row, &meta.type_name, &chrono::Local);
                    csv_text(&cell)
                })
                .collect();
            output
                .write_all(csv_line(&fields).as_bytes())
                .map_err(write_failed)?;
        }
        Ok(())
    })?;
    output.flush().map_err(write_failed)
}
