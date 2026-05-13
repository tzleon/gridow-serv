use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub qty: i32,
    pub location: String,
    pub location_id: Option<String>,
    pub category: String,
    pub tags: String,
    pub barcode: String,
    pub photos: String,
    pub photo_uri: String,
    pub buy_date: String,
    pub expiry: String,
    pub remark: String,
    pub track_low_stock: bool,
    pub owner_id: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ItemCreateRequest {
    pub name: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default = "default_qty")]
    pub qty: i32,
    pub location_id: Option<String>,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub barcode: String,
    #[serde(default)]
    pub photo_uri: String,
    #[serde(default)]
    pub buy_date: String,
    #[serde(default = "default_expiry")]
    pub expiry: String,
    #[serde(default)]
    pub remark: String,
    #[serde(default)]
    pub track_low_stock: bool,
}

#[derive(Debug, Deserialize)]
pub struct ItemUpdateRequest {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub qty: Option<i32>,
    pub location_id: Option<Option<String>>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub barcode: Option<String>,
    pub photo_uri: Option<String>,
    pub buy_date: Option<String>,
    pub expiry: Option<String>,
    pub remark: Option<String>,
    pub track_low_stock: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct OutboundRequest {
    pub qty: i32,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct TransferRequest {
    pub target_space_id: String,
    #[serde(default = "default_one")]
    pub qty: i32,
}

#[derive(Debug, Deserialize)]
pub struct ItemQueryParams {
    pub category: Option<String>,
    pub keyword: Option<String>,
    pub space_id: Option<String>,
    #[serde(default = "default_page")]
    pub page: i32,
    #[serde(default = "default_page_size")]
    pub page_size: i32,
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    #[serde(default = "default_sort_order")]
    pub sort_order: String,
}

#[derive(Debug, Serialize)]
pub struct ItemListResponse {
    pub items: Vec<Item>,
    pub total: i32,
    pub page: i32,
    pub page_size: i32,
}

fn default_icon() -> String {
    "📦".to_string()
}
fn default_qty() -> i32 {
    1
}
fn default_category() -> String {
    "daily".to_string()
}
fn default_expiry() -> String {
    "-".to_string()
}
fn default_page() -> i32 {
    1
}
fn default_page_size() -> i32 {
    20
}
fn default_sort_by() -> String {
    "updatedAt".to_string()
}
fn default_sort_order() -> String {
    "desc".to_string()
}
fn default_one() -> i32 {
    1
}

pub fn new_item_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn now_string() -> String {
    chrono::Utc::now().naive_utc().format("%Y-%m-%d %H:%M:%S").to_string()
}
