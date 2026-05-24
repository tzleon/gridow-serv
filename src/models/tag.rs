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
    pub version: i64,
    #[serde(rename = "is_deleted")]
    pub is_deleted: i16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncTagChange {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<Tag>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub updated: Vec<Tag>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deleted: Vec<String>,
}

impl SyncTagChange {
    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.updated.is_empty() && self.deleted.is_empty()
    }
    pub fn opt(self) -> Option<Self> {
        if self.is_empty() { None } else { Some(self) }
    }
}

#[derive(Debug, Deserialize)]
pub struct TagCreateRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct TagUpdateRequest {
    pub name: Option<String>,
}
