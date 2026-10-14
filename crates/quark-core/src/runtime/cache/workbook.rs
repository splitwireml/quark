//! Stages one workbook sheet as an all-text DuckDB table, following the `read_xlsx` rules in docs/plans/2026-10-13-xlsx-read-rules.md.

use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::{Context, Result, bail};
use calamine::{DataRef, Reader, Xlsx, XlsxCellReader};
use duckdb::{Connection, appender_params_from_iter};

use crate::shared::sql::quote_ident;

use self::text::{cell_text, column_letters};

type Book = Xlsx<BufReader<File>>;

/// The type `read_xlsx` declares for a column of date-styled cells; every other column is `Other`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Declared {
    Date,
    Timestamp,
    Time,
    Other,
}

/// The type `read_xlsx` declares for a column from its first data cell. calamine does not
/// expose the number format of a date cell, so the value decides between date, timestamp and time.
fn declare(cell: &DataRef<'_>) -> Declared {
    let DataRef::DateTime(stamp) = cell else {
        return Declared::Other;
    };
    let value = stamp.as_f64();
    if stamp.is_duration() || value < 1.0 {
        Declared::Time
    } else if value.fract() != 0.0 {
        Declared::Timestamp
    } else {
        Declared::Date
    }
}

/// One sheet row: its 0-based index and its non-empty cells as (0-based column, value).
struct Row<'a> {
    index: u32,
    cells: Vec<(u32, DataRef<'a>)>,
}

/// Groups the streamed cells of a sheet into rows, one row in memory at a time.
struct Rows<'a> {
    reader: XlsxCellReader<'a, BufReader<File>>,
    pending: Option<(u32, u32, DataRef<'a>)>,
    /// calamine fails when read again after the last cell.
    is_done: bool,
}

impl<'a> Rows<'a> {
    fn next_row(&mut self) -> Result<Option<Row<'a>>> {
        let mut row: Option<Row<'a>> = None;
        loop {
            let next = match self.pending.take() {
                Some(cell) => Some(cell),
                None if self.is_done => None,
                None => self.reader.next_cell()?.map(|cell| {
                    let (row, column) = cell.get_position();
                    (row, column, cell.get_value().clone())
                }),
            };
            let Some((index, column, value)) = next else {
                self.is_done = true;
                return Ok(row);
            };
            if row.as_ref().is_some_and(|current| current.index != index) {
                self.pending = Some((index, column, value));
                return Ok(row);
            }
            let row = row.get_or_insert_with(|| Row {
                index,
                cells: Vec::new(),
            });
            if cell_text(&value).is_some() {
                row.cells.push((column, value));
            }
        }
    }
}

/// The sheet names of a workbook, in workbook order.
pub fn sheet_names(path: &Path) -> Result<Vec<String>> {
    let names = open(path)?.sheet_names();
    if names.is_empty() {
        bail!("Workbook has no sheets");
    }
    Ok(names)
}

/// Streams `sheet` into a new all-`VARCHAR` table `table` and returns each column's name and declared type.
pub fn stage_sheet(
    conn: &Connection,
    path: &Path,
    sheet: &str,
    table: &str,
) -> Result<Vec<(String, Declared)>> {
    let mut book = open(path)?;
    let reader = book
        .worksheet_cells_reader(sheet)
        .with_context(|| format!("could not read sheet {sheet}"))?;
    let mut rows = Rows {
        reader,
        pending: None,
        is_done: false,
    };
    let (first, first_column, last_column) = loop {
        let Some(row) = rows.next_row()? else {
            bail!("Sheet has no cells");
        };
        if let (Some(&(low, _)), Some(&(high, _))) = (row.cells.first(), row.cells.last()) {
            break (row, low, high);
        }
    };
    let width = (last_column - first_column + 1) as usize;
    let has_header = first.cells.len() == width
        && first
            .cells
            .iter()
            .all(|(_, cell)| matches!(cell, DataRef::String(_) | DataRef::SharedString(_)));
    let names = if has_header {
        header_names(&first.cells)
    } else {
        (first_column..=last_column)
            .map(|column| format!("{}{}", column_letters(column), first.index + 1))
            .collect()
    };
    let columns: Vec<_> = names
        .iter()
        .map(|name| quote_ident(name) + " VARCHAR")
        .collect();
    let create = format!(
        "CREATE TABLE {} ({})",
        quote_ident(table),
        columns.join(", ")
    );
    conn.execute_batch(&create)?;

    let mut declared = vec![Declared::Other; width];
    let mut is_first_data_row = true;
    let mut appender = conn.appender(table)?;
    let mut append = |cells: &[(u32, DataRef<'_>)]| -> Result<()> {
        let mut values: Vec<Option<String>> = vec![None; width];
        for (column, cell) in cells {
            let slot = (column.wrapping_sub(first_column)) as usize;
            if slot < width {
                values[slot] = cell_text(cell);
                if is_first_data_row {
                    declared[slot] = declare(cell);
                }
            }
        }
        is_first_data_row = false;
        appender.append_row(appender_params_from_iter(values.iter()))?;
        Ok(())
    };
    let mut expected = first.index + 1;
    if !has_header {
        append(&first.cells)?;
    }
    while let Some(row) = rows.next_row()? {
        let is_in_range = row
            .cells
            .iter()
            .any(|(at, _)| (first_column..=last_column).contains(at));
        if row.index != expected || !is_in_range {
            break;
        }
        append(&row.cells)?;
        expected += 1;
    }
    appender.flush()?;
    Ok(names.into_iter().zip(declared).collect())
}

fn open(path: &Path) -> Result<Book> {
    let file = File::open(path).with_context(|| format!("could not open {}", path.display()))?;
    Xlsx::new(BufReader::new(file)).context("could not read the workbook")
}

/// Header cells as column names; a later name equal to an earlier one (ignoring case) gets `_<position>`.
fn header_names(cells: &[(u32, DataRef<'_>)]) -> Vec<String> {
    let mut used = HashSet::with_capacity(cells.len());
    (0..)
        .zip(cells)
        .map(|(position, (_, cell))| {
            let name = cell_text(cell).unwrap_or_default();
            if used.insert(name.to_lowercase()) {
                name
            } else {
                format!("{name}_{position}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
mod text;
