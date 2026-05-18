use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct HistoryRecord {
    #[serde(skip)]
    pub id: i64,
    #[serde(rename = "id")]
    pub public_id: String,
    pub r#type: String,
    #[serde(skip)]
    pub item_id: i64,
    pub item_name: String,
    pub qty: i32,
    pub from_location: Option<String>,
    pub to_location: Option<String>,
    pub reason: Option<String>,
    pub remark: Option<String>,
    pub time: String,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQueryParams {
    pub r#type: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i32,
    pub before: Option<String>,
    pub after: Option<String>,
}

fn default_limit() -> i32 { 20 }
