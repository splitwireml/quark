//! Recognizes row-wise projection chains and carries cached source positions through them.

use std::collections::BTreeSet;

use duckdb::Connection;
use serde_json::{Value, json};

use super::{cache_table, datasets, describe, internal};
use crate::error::ApiResult;
use crate::relation::guard::parse_select;
use crate::shared::sql::sql_string;

pub(super) fn cached(conn: &Connection, mut parsed: Value) -> ApiResult<Option<String>> {
    let node = &parsed["statements"][0]["node"];
    let mut functions = BTreeSet::new();
    let Some(base) = source(node, &mut functions) else {
        return Ok(None);
    };
    let (Some(schema), Some(name)) = (base["schema_name"].as_str(), base["table_name"].as_str())
    else {
        return Ok(None);
    };
    let listed = datasets(conn)
        .map_err(internal)?
        .into_iter()
        .any(|item| item.kind == "VIEW" && item.schema == schema && item.name == name);
    if !listed || !scalar_functions(conn, &functions)? {
        return Ok(None);
    }
    let Some(table) = cache_table(conn, schema, name)? else {
        return Ok(None);
    };
    let columns = describe(conn, &table, &[]).map_err(internal)?;
    if columns
        .iter()
        .any(|meta| meta.name.eq_ignore_ascii_case("rowid"))
    {
        return Ok(None);
    }
    if node["from_table"]["type"] == "BASE_TABLE" {
        return Ok(Some(table));
    }
    let cached = parse_select(conn, &format!("SELECT *, rowid FROM {table}"))?;
    carry_position(
        &mut parsed["statements"][0]["node"],
        &cached["statements"][0]["node"],
    );
    let sql: String = conn
        .query_row(
            "SELECT json_deserialize_sql(?)",
            [parsed.to_string()],
            |row| row.get(0),
        )
        .map_err(internal)?;
    Ok(Some(format!("({sql})")))
}

fn empty(value: &Value) -> bool {
    value.as_array().is_some_and(Vec::is_empty)
}

fn source<'a>(node: &'a Value, functions: &mut BTreeSet<String>) -> Option<&'a Value> {
    if node["type"] != "SELECT_NODE"
        || !empty(&node["modifiers"])
        || !empty(&node["cte_map"]["map"])
        || !empty(&node["group_expressions"])
        || !empty(&node["group_sets"])
        || node["aggregate_handling"] != "STANDARD_HANDLING"
        || ["where_clause", "having", "qualify", "sample"]
            .iter()
            .any(|key| !node[key].is_null())
    {
        return None;
    }
    let from = &node["from_table"];
    if !from["sample"].is_null() || !empty(&from["column_name_alias"]) {
        return None;
    }
    let select = node["select_list"].as_array()?;
    match from["type"].as_str()? {
        "BASE_TABLE"
            if select.len() == 1
                && plain_star(&select[0])
                && from["catalog_name"] == ""
                && from["alias"] == ""
                && from["at_clause"].is_null() =>
        {
            Some(from)
        }
        "SUBQUERY" if select.iter().all(|expr| row_wise(expr, functions)) => {
            source(&from["subquery"]["node"], functions)
        }
        _ => None,
    }
}

fn plain_star(expr: &Value) -> bool {
    expr["class"] == "STAR"
        && expr["relation_name"] == ""
        && expr["columns"] == false
        && expr["expr"].is_null()
        && [
            "exclude_list",
            "replace_list",
            "qualified_exclude_list",
            "rename_list",
        ]
        .iter()
        .all(|key| empty(&expr[key]))
}

fn row_wise(value: &Value, functions: &mut BTreeSet<String>) -> bool {
    // Unknown expressions stay generic: evaluating them after row selection may change results.
    match value {
        Value::Array(items) => items.iter().all(|item| row_wise(item, functions)),
        Value::Object(fields) => {
            if value["alias"]
                .as_str()
                .is_some_and(|alias| alias.eq_ignore_ascii_case("rowid"))
            {
                return false;
            }
            if let Some(class) = value["class"].as_str() {
                match class {
                    "COLUMN_REF" => {
                        if value["column_names"].as_array().is_none_or(|names| {
                            names
                                .last()
                                .and_then(Value::as_str)
                                .is_none_or(|name| name.eq_ignore_ascii_case("rowid"))
                        }) {
                            return false;
                        }
                    }
                    "FUNCTION" => {
                        if value["schema"] != ""
                            || value["catalog"] != ""
                            || value["distinct"] != false
                            || !value["filter"].is_null()
                            || !empty(&value["order_bys"]["orders"])
                            || value["export_state"] != false
                        {
                            return false;
                        }
                        let Some(name) = value["function_name"].as_str() else {
                            return false;
                        };
                        functions.insert(name.to_owned());
                    }
                    "OPERATOR"
                        if matches!(
                            value["type"].as_str(),
                            Some(
                                "OPERATOR_NOT"
                                    | "OPERATOR_IS_NULL"
                                    | "OPERATOR_IS_NOT_NULL"
                                    | "OPERATOR_COALESCE"
                                    | "COMPARE_IN"
                                    | "COMPARE_NOT_IN"
                            )
                        ) => {}
                    "CONSTANT" | "CAST" | "CASE" | "COMPARISON" | "CONJUNCTION" | "BETWEEN" => {}
                    _ => return false,
                }
            }
            fields.values().all(|item| row_wise(item, functions))
        }
        _ => true,
    }
}

fn scalar_functions(conn: &Connection, names: &BTreeSet<String>) -> ApiResult<bool> {
    if names.is_empty() {
        return Ok(true);
    }
    let names_sql = names
        .iter()
        .map(|name| sql_string(name))
        .collect::<Vec<_>>()
        .join(", ");
    let count: usize = conn
        .query_row(
            // Reject macros and ambiguous names as well as aggregates and volatile scalars.
            &format!(
                "SELECT count(*) FROM (SELECT function_name FROM duckdb_functions() \
             WHERE function_name IN ({names_sql}) GROUP BY function_name \
             HAVING bool_and(coalesce(function_type = 'scalar' AND stability = 'CONSISTENT' \
             AND internal AND NOT has_side_effects, false)))"
            ),
            [],
            |row| row.get(0),
        )
        .map_err(internal)?;
    Ok(count == names.len())
}

fn carry_position(node: &mut Value, cached: &Value) {
    if node["from_table"]["type"] == "BASE_TABLE" {
        *node = cached.clone();
    } else {
        carry_position(&mut node["from_table"]["subquery"]["node"], cached);
        if let Some(select) = node["select_list"].as_array_mut() {
            select.push(
                json!({"class": "COLUMN_REF", "type": "COLUMN_REF", "alias": "",
                "column_names": ["rowid"]}),
            );
        }
    }
}
