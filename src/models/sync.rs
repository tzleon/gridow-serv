use serde::{Deserialize, Serialize};
use super::history::HistoryRecord;
use super::item::Item;
use super::space::Space;

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPullResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<SyncEntityChange<Item>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spaces: Option<SyncEntityChange<Space>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<SyncHistoryChange>,
    pub server_time: String,
    pub has_more: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncEntityChange<T> {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<T>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub updated: Vec<T>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deleted: Vec<String>,
}

impl<T> SyncEntityChange<T> {
    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.updated.is_empty() && self.deleted.is_empty()
    }
    pub fn opt(self) -> Option<Self> {
        if self.is_empty() { None } else { Some(self) }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncHistoryChange {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<HistoryRecord>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deleted: Vec<String>,
}

impl SyncHistoryChange {
    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.deleted.is_empty()
    }
    pub fn opt(self) -> Option<Self> {
        if self.is_empty() { None } else { Some(self) }
    }
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<SyncConflict>,
    pub server_time: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assigned_items: Vec<IdVersionMapping>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assigned_spaces: Vec<IdVersionMapping>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assigned_history: Vec<IdVersionMapping>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IdVersionMapping {
    pub client_id: String,
    pub server_id: String,
    pub version: i64,
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

#[derive(Debug, Deserialize)]
pub struct SyncPullParams {
    pub local_version: Option<i64>,
}
