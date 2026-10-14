//! Per-engine LRU of numbered tables: a View's rows in scan order, each with its row number, so a find can resume after a cursor.

use super::results::{MEMORY_DATABASE, RESULTS_SCHEMA, table_name};

/// Numbered tables per engine; the least recently used one is dropped for a fifth.
const CAPACITY: usize = 4;

/// How a search gets at a View's rows: it builds the table, waits on nobody, or reads a finished one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberedState {
    /// The entry is new, or its last build failed: this search is the only creator and must build the table.
    Create,
    /// Another search is building the table: read the live numbered relation instead, never create it.
    Building,
    /// The table exists: read it.
    Ready,
}

/// What a search needs to read a View's numbered table, and the tables it pushed out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberedUse {
    /// `quark_mem.quark_results.f_<hash of sql>`.
    pub table: String,
    /// The row-number column, named from the same hash.
    pub row_key: String,
    pub state: NumberedState,
    /// Tables of the evicted entries, to drop through the primary connection.
    pub evicted: Vec<String>,
}

#[derive(Debug)]
pub struct NumberedCache {
    capacity: usize,
    /// Entries from least to most recently used.
    tables: Vec<Entry>,
}

#[derive(Debug)]
struct Entry {
    table: String,
    /// Set by the creator after the table's `CREATE` succeeded; until then the entry is building.
    is_ready: bool,
}

impl Default for NumberedCache {
    fn default() -> Self {
        Self::with_capacity(CAPACITY)
    }
}

/// Attaches the private in-memory database numbered tables and result tables live in.
pub fn memory_statement() -> String {
    format!(
        "ATTACH IF NOT EXISTS ':memory:' AS {MEMORY_DATABASE} (READ_WRITE);
         CREATE SCHEMA IF NOT EXISTS {RESULTS_SCHEMA}"
    )
}

impl NumberedCache {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity,
            tables: Vec::with_capacity(capacity + 1),
        }
    }

    /// Marks the table for `sql` as most recently used, adding it (and evicting the oldest) when unseen.
    /// An unseen entry is `Create` for exactly one caller and `Building` for the others until it is ready.
    /// `columns` are the View's column names, so the row-number column cannot shadow one of them.
    pub fn touch<'a>(
        &mut self,
        sql: &str,
        columns: impl IntoIterator<Item = &'a str>,
    ) -> NumberedUse {
        // `table_name` is `r_` plus the key's SHA-256 prefix; the same digest names the numbered table.
        let hash = &table_name(sql)[2..];
        let table = format!("{RESULTS_SCHEMA}.f_{hash}");
        let mut row_key = format!("__quark_row_{hash}");
        let names: Vec<&str> = columns.into_iter().collect();
        while names.contains(&row_key.as_str()) {
            row_key.push('_');
        }
        let known = self.tables.iter().position(|known| known.table == table);
        let (entry, state) = match known {
            Some(index) => {
                let entry = self.tables.remove(index);
                let state = if entry.is_ready {
                    NumberedState::Ready
                } else {
                    NumberedState::Building
                };
                (entry, state)
            }
            None => {
                let entry = Entry {
                    table: table.clone(),
                    is_ready: false,
                };
                (entry, NumberedState::Create)
            }
        };
        self.tables.push(entry);
        let excess = self.tables.len().saturating_sub(self.capacity);
        let evicted = self.tables.drain(..excess).map(|old| old.table).collect();
        NumberedUse {
            table,
            row_key,
            state,
            evicted,
        }
    }

    /// Records that the creator built the table, so later searches read it. False means the entry was
    /// evicted meanwhile and the table is an orphan the creator must drop.
    pub fn mark_ready(&mut self, table: &str) -> bool {
        match self.tables.iter_mut().find(|entry| entry.table == table) {
            Some(entry) => {
                entry.is_ready = true;
                true
            }
            None => false,
        }
    }

    /// Forgets a table whose creation failed, so the next search creates it again; a ready table stays.
    /// Only the creator calls this.
    pub fn forget(&mut self, table: &str) {
        self.tables
            .retain(|entry| entry.table != table || entry.is_ready);
    }
}
