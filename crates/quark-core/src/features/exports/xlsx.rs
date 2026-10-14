//! Writes the sheets as one XLSX workbook: a worksheet per sheet, a header row, then every row through the export cell rules.

use std::collections::HashSet;
use std::io::BufWriter;
use std::path::Path;
use std::sync::Arc;

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Timelike};
use duckdb::Connection;
use rust_xlsxwriter::{Format, Workbook, Worksheet, XlsxError};

use super::names::export_sheet_name;
use super::service::{ExportFile, PreparedSheet, read_to_end};
use super::stream::for_each_batch;
use crate::error::{ApiError, ApiResult};
use crate::relation::rows::select_list;
use crate::runtime::engine::Engine;
use crate::shared::encode::export::{ExportCell, export_cell, is_illegal_xml};

/// Excel's longest cell text.
const EXCEL_TEXT_LIMIT: usize = 32_767;
/// `Datelike::num_days_from_ce` of 1899-12-30, the epoch `openpyxl` counts days from.
const EXCEL_EPOCH_DAYS_FROM_CE: i32 = 693_594;
const SECONDS_PER_DAY: f64 = 86_400.0;

/// The number formats `openpyxl` reads back as dates, date-times, times and durations.
struct Formats {
    date: Format,
    date_time: Format,
    time: Format,
    duration: Format,
}

impl Formats {
    fn new() -> Self {
        Self {
            date: Format::new().set_num_format("yyyy-mm-dd"),
            date_time: Format::new().set_num_format("yyyy-mm-dd h:mm:ss"),
            time: Format::new().set_num_format("h:mm:ss"),
            duration: Format::new().set_num_format("[hh]:mm:ss"),
        }
    }
}

fn xlsx_failed(error: XlsxError) -> ApiError {
    tracing::error!(%error, "could not write a workbook");
    ApiError::internal("Internal error")
}

/// Writes every sheet, each read on its own node's engine, into a new workbook under `dir`.
/// A failed write deletes the file.
pub(super) fn write_xlsx(
    sheets: &[(Arc<Engine>, PreparedSheet)],
    dir: &Path,
) -> ApiResult<ExportFile> {
    let (guard, file) = ExportFile::create(dir, ".xlsx")?;
    let formats = Formats::new();
    let mut workbook = Workbook::new();
    workbook.set_tempdir(dir).map_err(xlsx_failed)?;
    let mut used = HashSet::new();
    for (engine, sheet) in sheets {
        let worksheet = workbook.add_worksheet_with_constant_memory();
        worksheet
            .set_name(export_sheet_name(&sheet.name, &mut used))
            .map_err(xlsx_failed)?;
        read_to_end(engine, |conn| write_sheet(conn, sheet, worksheet, &formats))?;
    }
    workbook
        .save_to_writer(BufWriter::new(file))
        .map_err(xlsx_failed)?;
    Ok(guard)
}

fn write_sheet(
    conn: &Connection,
    sheet: &PreparedSheet,
    worksheet: &mut Worksheet,
    formats: &Formats,
) -> ApiResult<()> {
    for (column, name) in (0..).zip(&sheet.header) {
        let name = name.chars().filter(|c| !is_illegal_xml(*c)).collect();
        write_cell(worksheet, 0, column, ExportCell::Text(name), formats)?;
    }
    let select = format!(
        "SELECT {} FROM ({})",
        select_list(&sheet.columns),
        sheet.sql
    );
    let mut row_number = 1;
    for_each_batch(conn, &select, |batch| {
        for row in 0..batch.num_rows() {
            for (column, (meta, array)) in (0..).zip(sheet.columns.iter().zip(batch.columns())) {
                let cell = export_cell(array.as_ref(), row, &meta.type_name, &chrono::Local);
                write_cell(worksheet, row_number, column, cell, formats)?;
            }
            row_number += 1;
        }
        Ok(())
    })?;
    Ok(())
}

fn write_cell(
    worksheet: &mut Worksheet,
    row: u32,
    column: u16,
    cell: ExportCell,
    formats: &Formats,
) -> ApiResult<()> {
    let written = match cell {
        ExportCell::Null => return Ok(()),
        ExportCell::Bool(flag) => worksheet.write_boolean(row, column, flag),
        // Integers beyond 10^15 are already text, so the conversion is exact.
        ExportCell::Int(number) => worksheet.write_number(row, column, number as f64),
        ExportCell::Float(number) => worksheet.write_number(row, column, number),
        ExportCell::Decimal(text) => match text.parse::<f64>() {
            Ok(number) => worksheet.write_number(row, column, number),
            Err(_) => worksheet.write_string(row, column, text),
        },
        ExportCell::Text(text) => {
            if text.len() > EXCEL_TEXT_LIMIT && text.chars().count() > EXCEL_TEXT_LIMIT {
                return Err(ApiError::unprocessable(format!(
                    "Excel cell text exceeds {EXCEL_TEXT_LIMIT} characters"
                )));
            }
            worksheet.write_string(row, column, text)
        }
        ExportCell::Date(date) => {
            worksheet.write_number_with_format(row, column, date_serial(date), &formats.date)
        }
        ExportCell::DateTime(moment) => worksheet.write_number_with_format(
            row,
            column,
            date_time_serial(moment),
            &formats.date_time,
        ),
        ExportCell::Time(time) => {
            worksheet.write_number_with_format(row, column, day_fraction(time), &formats.time)
        }
        ExportCell::Duration(span) => {
            worksheet.write_number_with_format(row, column, duration_days(span), &formats.duration)
        }
    };
    written.map(|_| ()).map_err(xlsx_failed)
}

/// Days since 1899-12-30, less one inside Excel's phantom 1900-02-29 window, as `openpyxl` writes them.
fn date_serial(date: NaiveDate) -> f64 {
    let days = date.num_days_from_ce() - EXCEL_EPOCH_DAYS_FROM_CE;
    f64::from(if (1..=60).contains(&days) {
        days - 1
    } else {
        days
    })
}

fn day_fraction(time: NaiveTime) -> f64 {
    (f64::from(time.num_seconds_from_midnight()) + f64::from(time.nanosecond()) / 1e9)
        / SECONDS_PER_DAY
}

fn date_time_serial(moment: NaiveDateTime) -> f64 {
    date_serial(moment.date()) + day_fraction(moment.time())
}

fn duration_days(span: TimeDelta) -> f64 {
    (span.num_seconds() as f64 + f64::from(span.subsec_nanos()) / 1e9) / SECONDS_PER_DAY
}

#[cfg(test)]
mod tests;
