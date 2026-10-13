//! Request and response types for projects.

use serde::{Deserialize, Serialize};

use crate::runtime::registry::ProjectRecord;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectSummary {
    #[serde(flatten)]
    pub project: ProjectRecord,
    pub source_count: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectCreate {
    pub name: String,
}
