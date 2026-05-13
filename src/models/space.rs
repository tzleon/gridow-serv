//! 空间数据模型
//!
//! 空间（Space）构成树形结构，通过 `parent_id` 建立父子关系。
//! `SpaceNode` 用于树形查询接口，包含子节点和物品 ID 列表。

use serde::{Deserialize, Serialize};

/// 空间数据库实体
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub icon: String,
    /// 空间下物品数量（由触发器或应用层维护）
    pub count: i32,
    /// 父空间 ID，顶级空间为 None
    pub parent_id: Option<String>,
    /// 树深度，顶级为 0
    pub depth: i32,
    /// 同级排序权重
    pub sort_order: i32,
    pub photo_uri: String,
    /// 所有者用户 ID
    pub owner_id: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 创建空间请求
#[derive(Debug, Deserialize)]
pub struct SpaceCreateRequest {
    pub name: String,
    #[serde(default = "default_space_icon")]
    pub icon: String,
    /// 父空间 ID，不传则创建顶级空间
    pub parent_id: Option<String>,
    #[serde(default)]
    pub photo_uri: String,
}

/// 更新空间请求（所有字段可选）
#[derive(Debug, Deserialize)]
pub struct SpaceUpdateRequest {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub photo_uri: Option<String>,
    pub sort_order: Option<i32>,
}

/// 空间树节点（递归结构）
///
/// 用于 `GET /v1/spaces/tree` 接口，返回完整树形结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceNode {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub count: i32,
    pub parent_id: Option<String>,
    pub depth: i32,
    pub photo_uri: String,
    /// 子空间列表
    pub children: Vec<SpaceNode>,
    /// 该空间下直接关联的物品 ID 列表
    pub item_ids: Vec<String>,
    pub owner_id: String,
}

/// 空间路径中的单段
#[derive(Debug, Serialize)]
pub struct SpacePathSegment {
    pub id: String,
    pub name: String,
    pub icon: String,
}

/// 空间路径响应
#[derive(Debug, Serialize)]
pub struct SpacePathResponse {
    /// 人类可读的路径，如 `"🏠 客厅 > 🗄️ 电视柜"`
    pub path: String,
    pub segments: Vec<SpacePathSegment>,
}

fn default_space_icon() -> String {
    "🏠".to_string()
}
