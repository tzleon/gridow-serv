use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tag {
    #[serde(skip)]
    pub id: i64,
    #[serde(rename = "id")]
    pub public_id: String,
    pub name: String,
    #[serde(skip)]
    pub owner_id: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct TagCreateRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct TagUpdateRequest {
    pub name: Option<String>,
}
