use serde::{Deserialize, Serialize};

use super::history::HistoryRecord;
use super::item::Item;
use super::space::Space;

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPullResponse {
    pub items: SyncEntityChange<Item>,
    pub spaces: SyncEntityChange<Space>,
    pub history: SyncHistoryChange,
    pub server_time: String,
    pub has_more: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncEntityChange<T> {
    pub created: Vec<T>,
    pub updated: Vec<T>,
    pub deleted: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncHistoryChange {
    pub created: Vec<HistoryRecord>,
    pub deleted: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct SyncPushRequest {
    pub items: Option<SyncEntityChange<Item>>,
    pub spaces: Option<SyncEntityChange<Space>>,
    pub history: Option<SyncHistoryChange>,
    pub client_time: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SyncPushResponse {
    pub success: bool,
    pub conflicts: Vec<SyncConflict>,
    pub server_time: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncConflict {
    pub entity: String,
    pub id: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct SyncStatusResponse {
    pub last_sync_time: Option<String>,
    pub pending_changes: i32,
    pub server_time: String,
}
