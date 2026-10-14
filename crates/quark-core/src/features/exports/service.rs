//! Export rules: the checks in Python's order, the sheet's columns, and the temporary file a writer fills.

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use duckdb::Connection;
use uuid::Uuid;

use super::copy::write_copy;
use super::csv::write_csv;
use super::model::{ExportFormat, ExportRequest, ExportSheet, JsonLayout};
use super::names::extension;
use super::xlsx::write_xlsx;
use crate::error::{ApiError, ApiResult};
use crate::relation::describe::describe;
use crate::relation::guard::check_select;
use crate::runtime::engine::Engine;
use crate::shared::columns::ColumnMeta;

/// A sheet whose SQL passed the checks. `columns` are the wrapped relation's names and types, which
/// DuckDB deduplicates, so they select the rows; `header` keeps the SQL's own result names.
pub struct PreparedSheet {
    pub(super) name: String,
    pub(super) sql: String,
    pub(super) columns: Vec<ColumnMeta>,
    pub(super) header: Vec<String>,
}

/// An export file under the exports folder. It is deleted when this guard drops, so every
/// failure, panic, finished download and abandoned download leaves nothing behind.
#[derive(Debug)]
pub struct ExportFile {
    path: PathBuf,
}

impl ExportFile {
    /// Creates `.export-<uuid><extension>` in `dir` and returns the open file with its guard.
    pub(super) fn create(dir: &Path, extension: &str) -> ApiResult<(Self, File)> {
        fs::create_dir_all(dir).map_err(write_failed)?;
        let path = dir.join(format!(".export-{}{extension}", Uuid::new_v4().simple()));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(write_failed)?;
        Ok((Self { path }, file))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ExportFile {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "could not delete an export file");
        }
    }
}

pub(super) fn write_failed(error: io::Error) -> ApiError {
    tracing::error!(%error, "could not write an export");
    ApiError::internal("Internal error")
}

/// A DuckDB error from the sheet's SQL, worded like Python's export handler.
pub(super) fn invalid_query(error: duckdb::Error) -> ApiError {
    ApiError::unprocessable(format!("Invalid SQL query: {error}"))
}

/// Every format but `xlsx` writes one sheet.
pub fn require_single_sheet(request: &ExportRequest) -> ApiResult<()> {
    if request.format != ExportFormat::Xlsx && request.sheets.len() != 1 {
        let format = match request.format {
            ExportFormat::Csv => "CSV",
            ExportFormat::Parquet => "PARQUET",
            ExportFormat::Json => "JSON",
            ExportFormat::Xlsx => "XLSX",
        };
        return Err(ApiError::unprocessable(format!(
            "{format} export requires exactly one sheet"
        )));
    }
    Ok(())
}

/// Checks the sheet's SQL is one read-only SELECT and reads its columns. Like Python's
/// `result.description`, the header names come from the unwrapped SQL, so `SELECT 1 a, 2 a` keeps both `a`.
pub fn prepare_sheet(conn: &Connection, sheet: &ExportSheet) -> ApiResult<PreparedSheet> {
    let sql = check_select(conn, &sheet.sql)?;
    let columns = describe(conn, &format!("({sql})"), &[]).map_err(invalid_query)?;
    let header = result_names(conn, &sql).map_err(invalid_query)?;
    Ok(PreparedSheet {
        name: sheet.name.clone(),
        sql,
        columns,
        header,
    })
}

/// The names a statement returns, in order, without DuckDB's renaming of repeats inside a subquery.
fn result_names(conn: &Connection, sql: &str) -> duckdb::Result<Vec<String>> {
    let mut statement = conn.prepare(&format!("DESCRIBE {sql}"))?;
    statement.query_map([], |row| row.get(0))?.collect()
}

/// Runs `work` on a pooled reader of `engine` to the end, as an export must.
pub(super) fn read_to_end<T>(
    engine: &Engine,
    work: impl FnOnce(&Connection) -> ApiResult<T>,
) -> ApiResult<T> {
    engine
        .read(None, work)
        .map_err(|_| ApiError::internal("Internal error"))?
}

/// Writes the sheets into a new file under `dir`, each read on its own node's engine; a failed
/// write deletes the file. Every format but `xlsx` writes the first sheet only.
pub fn write_export(
    format: ExportFormat,
    layout: JsonLayout,
    sheets: &[(Arc<Engine>, PreparedSheet)],
    dir: &Path,
) -> ApiResult<ExportFile> {
    match (format, sheets.first()) {
        (ExportFormat::Xlsx, _) => write_xlsx(sheets, dir),
        (ExportFormat::Csv, Some((engine, sheet))) => {
            let (guard, file) = ExportFile::create(dir, ".csv")?;
            read_to_end(engine, |conn| {
                write_csv(conn, sheet, &mut BufWriter::new(file))
            })?;
            Ok(guard)
        }
        (ExportFormat::Parquet | ExportFormat::Json, Some((engine, sheet))) => {
            let (guard, file) = ExportFile::create(dir, extension(format))?;
            drop(file);
            read_to_end(engine, |conn| {
                write_copy(conn, sheet, format, layout, guard.path(), dir)
            })?;
            Ok(guard)
        }
        _ => Err(ApiError::not_implemented("Not in the desktop build yet")),
    }
}
