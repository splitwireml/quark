//! Hidden dedupe positions and late fetching of only the cached rows kept on a page.

use crate::shared::columns::ColumnMeta;
use crate::shared::sql::quote_ident;

pub(super) fn position(columns: &[ColumnMeta]) -> String {
    let mut position = "__quark_position".to_owned();
    while columns
        .iter()
        .any(|meta| meta.name.eq_ignore_ascii_case(&position))
    {
        position.push('_');
    }
    quote_ident(&position)
}

pub(super) fn cached(
    table: &str,
    columns: &[ColumnMeta],
    position: &str,
    filters: &str,
    qualify: &str,
    order: &str,
) -> (String, String) {
    let select = columns
        .iter()
        .map(|meta| quote_ident(&meta.name))
        .collect::<Vec<_>>()
        .join(", ");
    let kept = format!("SELECT rowid AS {position} FROM {table}{filters}{qualify}");
    let fetch = |rows: &str| {
        format!("(SELECT rowid AS {position}, {select} FROM {table} WHERE rowid IN ({rows}))")
    };
    // Limit the narrow rowids before fetching payloads, including when nearly every key is unique.
    let page = fetch(&format!("{kept} ORDER BY {order} LIMIT ? OFFSET ?"));
    (
        fetch(&kept),
        format!("SELECT * EXCLUDE ({position}) FROM {page} ORDER BY {order}"),
    )
}
