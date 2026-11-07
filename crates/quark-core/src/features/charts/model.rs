//! Chart request types: the chart spec and the visualize requests that carry it.

use serde::Deserialize;

use crate::error::{ApiError, ApiResult};
use crate::relation::request::{Filter, QueryRequest, Sort};

const EXPRESSION_LIMIT: usize = 2000;
const LABEL_LIMIT: usize = 200;

fn default_page() -> i64 {
    QueryRequest::default().page
}

fn default_page_size() -> i64 {
    QueryRequest::default().page_size
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartEncodings {
    #[serde(default)]
    pub x: Option<String>,
    #[serde(default)]
    pub y: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartKind {
    Bar,
    Histogram,
    Box,
    Scatter,
    Line,
    Pie,
    Metric,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Metric {
    Count,
    Distinct,
    Min,
    Max,
    Sum,
    Avg,
    Median,
    Stddev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    Grouped,
    Stacked,
    Stacked100,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Grain {
    Hour,
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Font {
    Sans,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Size {
    Fit,
    Sm,
    Md,
    Lg,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartSpec {
    pub chart: ChartKind,
    #[serde(default)]
    pub encodings: ChartEncodings,
    #[serde(default)]
    pub metric: Option<Metric>,
    #[serde(default)]
    pub density: bool,
    #[serde(default)]
    pub layout: Option<Layout>,
    #[serde(default)]
    pub grain: Option<Grain>,
    #[serde(default)]
    pub expression: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub align: Option<Align>,
    #[serde(default)]
    pub font: Option<Font>,
    #[serde(default)]
    pub size: Option<Size>,
}

impl ChartSpec {
    /// Rejects an `expression` over 2000 characters or a `label` over 200 with a 422.
    pub fn validate(&self) -> ApiResult<()> {
        if self
            .expression
            .as_ref()
            .is_some_and(|text| text.chars().count() > EXPRESSION_LIMIT)
        {
            return Err(ApiError::unprocessable("Metric expression is too long"));
        }
        if self
            .label
            .as_ref()
            .is_some_and(|text| text.chars().count() > LABEL_LIMIT)
        {
            return Err(ApiError::unprocessable("Metric label is too long"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisualizeRequest {
    pub spec: ChartSpec,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub sorts: Vec<Sort>,
    #[serde(default)]
    pub dedupe_columns: Vec<String>,
}

impl VisualizeRequest {
    pub fn query(&self) -> QueryRequest {
        QueryRequest {
            page: self.page,
            page_size: self.page_size,
            filters: self.filters.clone(),
            sorts: self.sorts.clone(),
            dedupe_columns: self.dedupe_columns.clone(),
        }
    }

    /// Checks the paging, then the spec's text limits.
    pub fn validate(&self) -> ApiResult<()> {
        self.query().validate()?;
        self.spec.validate()
    }
}

/// A [`VisualizeRequest`] plus the SQL text. The fields are repeated because serde
/// cannot combine `flatten` with `deny_unknown_fields`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SqlVisualizeRequest {
    pub sql: String,
    pub spec: ChartSpec,
    #[serde(default = "default_page")]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    #[serde(default)]
    pub filters: Vec<Filter>,
    #[serde(default)]
    pub sorts: Vec<Sort>,
    #[serde(default)]
    pub dedupe_columns: Vec<String>,
}

impl SqlVisualizeRequest {
    pub fn query(&self) -> QueryRequest {
        QueryRequest {
            page: self.page,
            page_size: self.page_size,
            filters: self.filters.clone(),
            sorts: self.sorts.clone(),
            dedupe_columns: self.dedupe_columns.clone(),
        }
    }

    /// Checks the paging, then the spec's text limits.
    pub fn validate(&self) -> ApiResult<()> {
        self.query().validate()?;
        self.spec.validate()
    }
}

/// The aggregate SQL for a metric over an already quoted column.
pub fn metric_sql(metric: Metric, column: &str) -> String {
    match metric {
        Metric::Count => format!("count({column})"),
        Metric::Distinct => format!("count(DISTINCT {column})"),
        Metric::Min => format!("min({column})"),
        Metric::Max => format!("max({column})"),
        Metric::Sum => format!("sum({column})"),
        Metric::Avg => format!("avg({column})"),
        Metric::Median => format!("median({column})"),
        Metric::Stddev => format!("stddev_samp({column})"),
    }
}

#[cfg(test)]
mod tests;
