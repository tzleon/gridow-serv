//! 物品数据模型
//!
//! `Item` 为数据库实体，`ItemCreateRequest` / `ItemUpdateRequest` 为 API 请求体，
//! `ItemQueryParams` 支持分页、排序、分类筛选和关键字搜索。
//! 所有日期/时间字段以 `"YYYY-MM-DD HH:MM:SS"` 格式存储。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 物品数据库实体
///
/// 通过 `sqlx::FromRow` 可从查询结果直接反序列化。
/// `tags` / `photos` 字段以 JSON 数组字符串形式存储。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Item {
    pub id: String,
    pub name: String,
    /// 图标 emoji，默认 "📦"
    pub icon: String,
    /// 当前库存数量
    pub qty: i32,
    /// 所在空间路径（冗余字段，格式 "🏠 客厅 > 🗄️ 电视柜"）
    pub location: String,
    /// 所在空间 ID（可为空）
    pub location_id: Option<String>,
    /// 分类：daily/food/tool/medicine/clothes/electronics
    pub category: String,
    /// 标签 JSON 数组字符串，如 `'["清洁","日用品"]'`
    pub tags: String,
    /// 条码/二维码内容
    pub barcode: String,
    /// 照片 URL 列表 JSON 数组字符串
    pub photos: String,
    /// 主图 URL
    pub photo_uri: String,
    /// 购买日期
    pub buy_date: String,
    /// 过期日期，`"-"` 表示无过期
    pub expiry: String,
    /// 备注
    pub remark: String,
    /// 是否跟踪低库存
    pub track_low_stock: bool,
    /// 所有者用户 ID
    pub owner_id: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建物品请求
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

/// 更新物品请求（所有字段可选，不传则保留原值）
///
/// `location_id` 类型为 `Option<Option<String>>`：
/// * `None` — 不修改
/// * `Some(None)` — 清空
/// * `Some(Some(id))` — 设置为指定 ID
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

/// 出库请求
#[derive(Debug, Deserialize)]
pub struct OutboundRequest {
    /// 出库数量（≥1）
    pub qty: i32,
    #[serde(default)]
    pub reason: String,
}

/// 物品转移请求
#[derive(Debug, Deserialize)]
pub struct TransferRequest {
    /// 目标空间 ID
    pub target_space_id: String,
    #[serde(default = "default_one")]
    pub qty: i32,
}

/// 物品列表查询参数
///
/// 支持分页、排序、分类筛选、关键字模糊搜索。
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

/// 物品列表响应（含分页信息）
#[derive(Debug, Serialize)]
pub struct ItemListResponse {
    pub items: Vec<Item>,
    pub total: i32,
    pub page: i32,
    pub page_size: i32,
}

// ── 默认值函数 ──────────────────────────────────────────────

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

/// 生成 UUID v4 作为物品 ID
pub fn new_item_id() -> String {
    Uuid::new_v4().to_string()
}

/// 获取当前 UTC 时间的格式化字符串 `"YYYY-MM-DD HH:MM:SS"`
pub fn now_string() -> String {
    chrono::Utc::now().naive_utc().format("%Y-%m-%d %H:%M:%S").to_string()
}
