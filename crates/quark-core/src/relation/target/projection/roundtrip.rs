//! Rejects rendered projection SQL when its parsed meaning or visible schema changes.

use duckdb::Connection;
use serde_json::Value;

use crate::relation::describe::describe;
use crate::relation::guard::parse_select;
use crate::shared::columns::ColumnMeta;

pub(super) fn preserves(
    conn: &Connection,
    mut intended: Value,
    sql: &str,
    visible: &[ColumnMeta],
) -> bool {
    let Ok(mut rendered) = parse_select(conn, sql) else {
        return false;
    };
    // The intended tree already includes the cache substitution, with the user's expressions intact.
    normalize(&mut intended);
    normalize(&mut rendered);
    if intended != rendered {
        return false;
    }
    let Ok(mut columns) = describe(conn, &format!("({sql})"), &[]) else {
        return false;
    };
    // Position is appended to the projection; every visible name and type must still match exactly.
    columns
        .pop()
        .is_some_and(|column| column.name == "rowid" && column.type_name == "BIGINT")
        && columns == visible
}

fn normalize(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            fields.remove("query_location");
            if let Some(Value::Array(select)) = fields.get_mut("select_list") {
                // The classifier forbids user rowid references, so these are only injected positions.
                select.retain(|expr| {
                    !(expr["class"] == "COLUMN_REF"
                        && expr["column_names"]
                            .as_array()
                            .is_some_and(|names| names.len() == 1 && names[0] == "rowid"))
                });
            }
            fields.values_mut().for_each(normalize);
        }
        Value::Array(items) => items.iter_mut().for_each(normalize),
        _ => {}
    }
}
