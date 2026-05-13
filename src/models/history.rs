//! 操作历史模型
//!
//! 每次入库（in）、出库（out）、转移（move）操作都会生成一条历史记录。
//! `r#type` 使用 raw identifier 避免与 Rust 关键字 `type` 冲突。

use serde::{Deserialize, Serialize};

/// 操作历史数据库实体
///
/// 通过 `sqlx::FromRow` 可从查询结果直接反序列化。
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct HistoryRecord {
    pub id: String,
    /// 操作类型：`in` / `out` / `move`
    pub r#type: String,
    pub item_id: String,
    pub item_name: String,
    /// 操作数量
    pub qty: i32,
    /// 来源位置（出库/转移时）
    pub from_location: Option<String>,
    /// 目标位置（入库/转移时）
    pub to_location: Option<String>,
    /// 原因说明
    pub reason: Option<String>,
    pub remark: Option<String>,
    pub time: String,
}

/// 历史记录查询参数
///
/// 支持按类型筛选、游标分页（`before` + `limit`）。
#[derive(Debug, Deserialize)]
pub struct HistoryQueryParams {
    pub r#type: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i32,
    /// 游标：查询此时间之前的记录
    pub before: Option<String>,
}

fn default_limit() -> i32 {
    20
}
