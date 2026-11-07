//! Imports a workbook once into a typed DuckDB cache file, with the column rules of `_xlsx_table` in the Python backend.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use duckdb::Connection;

use super::columnar_files::remove_partial;
use super::columnar_key::ColumnarKey;
use super::workbook::{Declared, stage_sheet};
use crate::runtime::dirs::{Dirs, configure_spill};
use crate::runtime::mount::engine_version;
use crate::shared::sql::quote_ident;

/// The name of the all-text table a sheet is staged in before its typed copy is made.
const STAGE: &str = "quark_stage";

/// At least this share of a column's non-blank texts must cast for the column to take that type.
const CAST_SHARE: f64 = 0.9;

/// Imports run one at a time, so two callers never share one partial file.
static IMPORTING: Mutex<()> = Mutex::new(());

/// What `_xlsx_table` measures about one text column.
#[derive(Debug, Default)]
struct Ratios {
    numeric: Option<f64>,
    date: Option<f64>,
    timestamp: Option<f64>,
    has_time: bool,
    has_fraction: bool,
}

/// The cache key of `sheets` of the workbook at `path`: the file's identity plus the sheet list.
pub fn workbook_key(path: &Path, sheets: &[String]) -> io::Result<ColumnarKey> {
    let list = serde_json::to_string(sheets).map_err(io::Error::other)?;
    ColumnarKey::new(path, &format!("xlsx:{list}"), engine_version())
}

/// Imports `sheets` into the workbook's cache file unless it is already there, and returns its path.
pub fn import_workbook(path: &Path, sheets: &[String], cache_dir: &Path) -> Result<PathBuf> {
    let key = workbook_key(path, sheets).context("could not read the workbook")?;
    let file = key.final_path(cache_dir);
    let _turn = IMPORTING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if key.is_ready(cache_dir) {
        return Ok(file);
    }
    let partial = key.partial_path(cache_dir);
    fs::create_dir_all(cache_dir.join("columnar")).context("could not create the cache folder")?;
    remove_partial(&partial);
    if let Err(error) = build(path, sheets, cache_dir, &partial) {
        remove_partial(&partial);
        return Err(error);
    }
    fs::rename(&partial, &file).context("could not publish the workbook cache")?;
    Ok(file)
}

fn build(path: &Path, sheets: &[String], cache_dir: &Path, partial: &Path) -> Result<()> {
    let conn = Connection::open(partial).context("could not create the workbook cache")?;
    let dirs = Dirs {
        data: cache_dir.to_owned(),
        cache: cache_dir.to_owned(),
    };
    configure_spill(&conn, &dirs)?;
    let stage = quote_ident(STAGE);
    for sheet in sheets {
        let columns = stage_sheet(&conn, path, sheet, STAGE)?;
        let ratios = measure(&conn, &columns)?;
        let selects: Vec<_> = columns
            .iter()
            .zip(&ratios)
            .map(|((name, declared), ratios)| column_expression(name, *declared, ratios))
            .collect();
        conn.execute_batch(&format!(
            "CREATE TABLE main.{} AS SELECT {} FROM {stage}; DROP TABLE {stage}",
            quote_ident(sheet),
            selects.join(", ")
        ))?;
    }
    conn.execute_batch("CHECKPOINT")?;
    conn.close()
        .map_err(|(_, error)| error)
        .context("could not close the workbook cache")
}

/// The five measures for every staged column, from one scan of the staging table.
fn measure(conn: &Connection, columns: &[(String, Declared)]) -> Result<Vec<Ratios>> {
    let measures: Vec<_> = columns.iter().map(|(name, _)| measure_sql(name)).collect();
    let sql = format!("SELECT {} FROM {}", measures.join(", "), quote_ident(STAGE));
    Ok(conn.query_row(&sql, [], |row| {
        (0..columns.len())
            .map(|at| {
                let base = at * 5;
                Ok(Ratios {
                    numeric: row.get(base)?,
                    date: row.get(base + 1)?,
                    timestamp: row.get(base + 2)?,
                    has_time: row.get::<_, i64>(base + 3)? > 0,
                    has_fraction: row.get::<_, i64>(base + 4)? > 0,
                })
            })
            .collect::<duckdb::Result<Vec<_>>>()
    })?)
}

fn measure_sql(name: &str) -> String {
    let c = quote_ident(name);
    let filled = format!("NULLIF(COUNT(*) FILTER (WHERE {c} IS NOT NULL AND TRIM({c}) != ''), 0)");
    let ratio = |kind: &str| {
        format!(
            "CAST(COUNT(*) FILTER (WHERE TRY_CAST({c} AS {kind}) IS NOT NULL) * 1.0 / {filled} AS DOUBLE)"
        )
    };
    format!(
        "{}, {}, {}, COUNT(*) FILTER (WHERE regexp_matches(TRIM({c}), '[T ]\\d{{1,2}}:')), \
         COUNT(*) FILTER (WHERE TRY_CAST({c} AS DOUBLE) IS NOT NULL AND TRY_CAST({c} AS DOUBLE) != floor(TRY_CAST({c} AS DOUBLE)))",
        ratio("DOUBLE"),
        ratio("DATE"),
        ratio("TIMESTAMP")
    )
}

/// The select item that types one staged column, with Python's rules in Python's order.
fn column_expression(name: &str, declared: Declared, ratios: &Ratios) -> String {
    let c = quote_ident(name);
    let serial = format!("TIMESTAMP '1899-12-30' + TRY_CAST({c} AS DOUBLE) * INTERVAL 1 DAY");
    let is_enough = |ratio: Option<f64>| ratio.is_some_and(|share| share >= CAST_SHARE);
    let typed = match declared {
        Declared::Date if ratios.has_fraction => serial,
        Declared::Date => format!("CAST({serial} AS DATE)"),
        Declared::Timestamp => serial,
        Declared::Time => format!("CAST({serial} AS TIME)"),
        Declared::Other if is_enough(ratios.numeric) => format!("TRY_CAST({c} AS DOUBLE)"),
        Declared::Other if is_enough(ratios.timestamp) && ratios.has_time => {
            format!("TRY_CAST({c} AS TIMESTAMP)")
        }
        Declared::Other if is_enough(ratios.date) => format!("TRY_CAST({c} AS DATE)"),
        Declared::Other => c.clone(),
    };
    format!("{typed} AS {c}")
}

#[cfg(test)]
mod tests;
