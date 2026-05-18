use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Space {
    #[serde(skip)]
    pub id: i64,
    #[serde(rename = "id")]
    pub public_id: String,
    pub name: String,
    pub icon: String,
    pub count: i32,
    #[serde(skip)]
    pub parent_id: Option<i64>,
    #[serde(rename = "parent_id")]
    #[sqlx(default)]
    pub parent_public_id: Option<String>,
    pub depth: i32,
    pub sort_order: i32,
    pub photo_uri: String,
    #[serde(skip)]
    pub owner_id: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SpaceCreateRequest {
    pub name: String,
    #[serde(default = "default_space_icon")]
    pub icon: String,
    pub parent_id: Option<String>,
    #[serde(default)]
    pub photo_uri: String,
}

#[derive(Debug, Deserialize)]
pub struct SpaceUpdateRequest {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub photo_uri: Option<String>,
    pub sort_order: Option<i32>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceNode {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub count: i32,
    pub parent_id: Option<String>,
    pub depth: i32,
    pub photo_uri: String,
    pub children: Vec<SpaceNode>,
    pub item_ids: Vec<String>,
    pub owner_id: String,
}

#[derive(Debug, Serialize)]
pub struct SpacePathSegment {
    pub id: String,
    pub name: String,
    pub icon: String,
}

#[derive(Debug, Serialize)]
pub struct SpacePathResponse {
    pub path: String,
    pub segments: Vec<SpacePathSegment>,
}

fn default_space_icon() -> String { "🏠".to_string() }
