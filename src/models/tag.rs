use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub owner_id: String,
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
