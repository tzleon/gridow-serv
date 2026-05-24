use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Category {
    #[serde(skip)]
    pub id: i64,
    #[serde(rename = "id")]
    pub public_id: String,
    pub name: String,
    pub icon: String,
    pub sort_order: i32,
    #[serde(skip)]
    pub owner_id: i64,
    pub created_at: String,
    #[sqlx(default)]
    #[serde(default)]
    pub item_count: i64,
    #[sqlx(default)]
    #[serde(default)]
    pub last_used_at: Option<String>,
    pub version: i64,
    #[serde(rename = "is_deleted")]
    pub is_deleted: i16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncCategoryChange {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<Category>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub updated: Vec<Category>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deleted: Vec<String>,
}

impl SyncCategoryChange {
    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.updated.is_empty() && self.deleted.is_empty()
    }
    pub fn opt(self) -> Option<Self> {
        if self.is_empty() { None } else { Some(self) }
    }
}

#[derive(Debug, Deserialize)]
pub struct CategoryCreateRequest {
    pub name: String,
    #[serde(default = "default_icon")]
    pub icon: String,
}

#[derive(Debug, Deserialize)]
pub struct CategoryUpdateRequest {
    pub name: Option<String>,
    pub icon: Option<String>,
}

fn default_icon() -> String { "📦".to_string() }
