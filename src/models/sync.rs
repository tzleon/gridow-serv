//! 数据同步模型
//!
//! 支持离线客户端与服务端的增量数据同步。
//! * `sync_pull` — 客户端拉取服务端增量变更（仅返回登录用户的授权数据）
//! * `sync_push` — 客户端推送本地变更到服务端（含冲突检测）
//!
//! 同步基于时间戳比较，`last_sync_time` 作为增量边界。

use serde::{Deserialize, Serialize};

use super::history::HistoryRecord;
use super::item::Item;
use super::space::Space;

/// 同步拉取响应 — 包含三类实体的增量变更
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPullResponse {
    pub items: SyncEntityChange<Item>,
    pub spaces: SyncEntityChange<Space>,
    pub history: SyncHistoryChange,
    /// 服务端当前时间，客户端应保存作为下次 `last_sync_time`
    pub server_time: String,
    /// 是否还有更多数据（当前实现始终为 false）
    pub has_more: bool,
}

/// 实体变更集合（泛型，适配 Item 和 Space）
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncEntityChange<T> {
    pub created: Vec<T>,
    pub updated: Vec<T>,
    /// 被删除的实体 ID 列表
    pub deleted: Vec<String>,
}

/// 历史记录的变更集合
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncHistoryChange {
    pub created: Vec<HistoryRecord>,
    pub deleted: Vec<String>,
}

/// 同步推送请求 — 包含三类实体的本地变更
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SyncPushRequest {
    pub items: Option<SyncEntityChange<Item>>,
    pub spaces: Option<SyncEntityChange<Space>>,
    pub history: Option<SyncHistoryChange>,
    /// 客户端当前时间
    pub client_time: Option<String>,
}

/// 同步推送响应
#[derive(Debug, Serialize)]
pub struct SyncPushResponse {
    pub success: bool,
    /// 冲突列表（ID 重复等）
    pub conflicts: Vec<SyncConflict>,
    pub server_time: String,
}

/// 同步冲突详情
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncConflict {
    pub entity: String,
    pub id: String,
    /// 冲突原因：`version_mismatch` / `deleted_remotely` / `modified_remotely`
    pub reason: String,
}

/// 同步状态响应
#[derive(Debug, Serialize)]
pub struct SyncStatusResponse {
    pub last_sync_time: Option<String>,
    pub pending_changes: i32,
    pub server_time: String,
}
