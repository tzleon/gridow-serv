use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub sort_order: i32,
    pub owner_id: String,
    pub created_at: String,
    #[sqlx(default)]
    #[serde(default)]
    pub item_count: i64,
    #[sqlx(default)]
    #[serde(default)]
    pub last_used_at: Option<String>,
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

fn default_icon() -> String {
    "📦".to_string()
}
