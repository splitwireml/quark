//! Column lookups and the SQL fragments that decide which rows and fields a scatter reads.

use crate::error::{ApiError, ApiResult};
use crate::relation::filtered::Relation;
use crate::shared::columns::ColumnMeta;
use crate::shared::sql::quote_ident;
use crate::shared::types::{is_timetz, profile_kind};

pub(super) fn find<'a>(relation: &'a Relation, name: Option<&str>) -> Option<&'a ColumnMeta> {
    let name = name.filter(|name| !name.is_empty())?;
    relation.columns.iter().find(|column| column.name == name)
}

/// An optional encoding: absent stays absent, a named column that is not there is a 422.
pub(super) fn role<'a>(
    relation: &'a Relation,
    role: &str,
    name: Option<&str>,
) -> ApiResult<Option<&'a ColumnMeta>> {
    if name.is_none_or(str::is_empty) {
        return Ok(None);
    }
    find(relation, name)
        .map(Some)
        .ok_or_else(|| ApiError::unprocessable(format!("Scatter {role} column was not found")))
}

pub(super) fn is_numeric(meta: &ColumnMeta) -> bool {
    profile_kind(&meta.type_name) == Some("numeric")
}

/// The column as the client reads it: TIMETZ goes out as text because Arrow drops its offset.
pub(super) fn field(meta: &ColumnMeta) -> String {
    let name = quote_ident(&meta.name);
    if is_timetz(&meta.type_name) {
        format!("CAST({name} AS VARCHAR)")
    } else {
        name
    }
}

/// A coordinate that can be drawn: non-null and, for numbers, finite.
pub(super) fn usable(meta: &ColumnMeta) -> String {
    let name = quote_ident(&meta.name);
    if is_numeric(meta) {
        usable_value(&name)
    } else {
        format!("{name} IS NOT NULL")
    }
}

pub(super) fn usable_value(name: &str) -> String {
    format!("{name} IS NOT NULL AND isfinite({name}::DOUBLE)")
}
