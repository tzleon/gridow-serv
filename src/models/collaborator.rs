//! 协管关系模型
//!
//! 协管（Collaborator）表示所有者授权其他用户共同管理某个物品或空间。
//! 一条记录关联一个实体（item/space）和一个被授权的用户。

use serde::{Deserialize, Serialize};

/// 协管关系数据库实体
///
/// 唯一约束 `(entity_type, entity_id, user_id)` 防止重复授权。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Collaborator {
    pub id: String,
    /// 实体类型：`item` 或 `space`
    pub entity_type: String,
    /// 实体 ID
    pub entity_id: String,
    /// 被授权用户 ID
    pub user_id: String,
    pub created_at: String,
}

/// 添加协管请求
#[derive(Debug, Deserialize)]
pub struct AddCollaboratorRequest {
    /// 要添加为协管的用户 ID
    pub user_id: String,
}

/// 协管列表响应
#[derive(Debug, Serialize)]
pub struct CollaboratorListResponse {
    pub collaborators: Vec<CollaboratorInfo>,
}

/// 协管详细信息（JOIN users 表获取用户名等）
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CollaboratorInfo {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub avatar: String,
}
