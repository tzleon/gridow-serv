use serde::{Deserialize, Serialize};
use super::category::SyncCategoryChange;
use super::history::HistoryRecord;
use super::item::Item;
use super::space::Space;
use super::tag::SyncTagChange;

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPullResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<SyncEntityChange<Item>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spaces: Option<SyncEntityChange<Space>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<SyncHistoryChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<SyncCategoryChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<SyncTagChange>,
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
    pub categories: Option<SyncCategoryChange>,
    pub tags: Option<SyncTagChange>,
    pub client_time: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SyncPushResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<SyncConflict>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assigned_items: Vec<IdVersionMapping>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assigned_spaces: Vec<IdVersionMapping>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assigned_history: Vec<IdVersionMapping>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assigned_categories: Vec<IdVersionMapping>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assigned_tags: Vec<IdVersionMapping>,
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
}

#[derive(Debug, Deserialize)]
pub struct SyncPullParams {
    pub local_version: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── SyncEntityChange ─────────────────────────────────

    #[test]
    fn test_sync_entity_change_is_empty_all_empty() {
        let change: SyncEntityChange<Item> = SyncEntityChange {
            created: vec![],
            updated: vec![],
            deleted: vec![],
        };
        assert!(change.is_empty());
    }

    #[test]
    fn test_sync_entity_change_is_empty_with_created() {
        let change: SyncEntityChange<Item> = SyncEntityChange {
            created: vec![Item { public_id: "abc".into(), name: "test".into(), ..Default::default() }],
            updated: vec![],
            deleted: vec![],
        };
        assert!(!change.is_empty());
    }

    #[test]
    fn test_sync_entity_change_is_empty_with_updated() {
        let change: SyncEntityChange<Item> = SyncEntityChange {
            created: vec![],
            updated: vec![Item { public_id: "abc".into(), name: "test".into(), ..Default::default() }],
            deleted: vec![],
        };
        assert!(!change.is_empty());
    }

    #[test]
    fn test_sync_entity_change_is_empty_with_deleted() {
        let change: SyncEntityChange<Item> = SyncEntityChange {
            created: vec![],
            updated: vec![],
            deleted: vec!["id1".into()],
        };
        assert!(!change.is_empty());
    }

    #[test]
    fn test_sync_entity_change_opt_returns_none_when_empty() {
        let change: SyncEntityChange<Item> = SyncEntityChange {
            created: vec![],
            updated: vec![],
            deleted: vec![],
        };
        assert!(change.opt().is_none());
    }

    #[test]
    fn test_sync_entity_change_opt_returns_some_when_not_empty() {
        let change: SyncEntityChange<Item> = SyncEntityChange {
            created: vec![],
            updated: vec![],
            deleted: vec!["id1".into()],
        };
        assert!(change.opt().is_some());
    }

    // ── SyncHistoryChange ────────────────────────────────

    #[test]
    fn test_sync_history_change_is_empty_all_empty() {
        let change = SyncHistoryChange {
            created: vec![],
            deleted: vec![],
        };
        assert!(change.is_empty());
    }

    #[test]
    fn test_sync_history_change_is_empty_with_created() {
        let change = SyncHistoryChange {
            created: vec![HistoryRecord {
                public_id: "h1".into(), r#type: "in".into(),
                item_public_id: "i1".into(), item_name: "test".into(),
                qty: 1, time: "2024-01-01".into(), ..Default::default()
            }],
            deleted: vec![],
        };
        assert!(!change.is_empty());
    }

    #[test]
    fn test_sync_history_change_is_empty_with_deleted() {
        let change = SyncHistoryChange {
            created: vec![],
            deleted: vec!["h1".into()],
        };
        assert!(!change.is_empty());
    }

    #[test]
    fn test_sync_history_change_opt_returns_none_when_empty() {
        let change = SyncHistoryChange {
            created: vec![],
            deleted: vec![],
        };
        assert!(change.opt().is_none());
    }

    #[test]
    fn test_sync_history_change_opt_returns_some_when_not_empty() {
        let change = SyncHistoryChange {
            created: vec![],
            deleted: vec!["h1".into()],
        };
        assert!(change.opt().is_some());
    }

    // ── SyncPullResponse / SyncPushResponse ──────────────

    #[test]
    fn test_sync_pull_response_serialization() {
        let resp = SyncPullResponse {
            items: None,
            spaces: None,
            history: None,
            categories: None,
            tags: None,
            has_more: false,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""has_more":false"#));
        // 所有 Option 为 None 的字段应被跳过
        assert!(!json.contains("items"));
        assert!(!json.contains("spaces"));
    }

    #[test]
    fn test_sync_pull_response_with_data() {
        let change = SyncEntityChange::<Item> {
            created: vec![Item {
                public_id: "abc".into(), name: "item1".into(), icon: "📦".into(),
                qty: 5, location: "room".into(), category: "daily".into(),
                created_at: "now".into(), updated_at: "now".into(), ..Default::default()
            }],
            updated: vec![],
            deleted: vec![],
        };

        let resp = SyncPullResponse {
            items: Some(change),
            spaces: None,
            history: None,
            categories: None,
            tags: None,
            has_more: false,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("item1"));
        assert!(json.contains("abc"));
    }

    #[test]
    fn test_id_version_mapping_serialization() {
        let mapping = IdVersionMapping {
            client_id: "client1".into(),
            server_id: "server1".into(),
            version: 42,
        };
        let json = serde_json::to_string(&mapping).unwrap();
        assert!(json.contains("client1"));
        assert!(json.contains("server1"));
        assert!(json.contains("42"));
    }

    #[test]
    fn test_sync_conflict_serialization() {
        let conflict = SyncConflict {
            entity: "item".into(),
            id: "abc123".into(),
            reason: "version conflict".into(),
        };
        let json = serde_json::to_string(&conflict).unwrap();
        assert!(json.contains("item"));
        assert!(json.contains("abc123"));
    }

    #[test]
    fn test_sync_status_response_serialization() {
        let status = SyncStatusResponse {
            last_sync_time: Some("2024-01-01T00:00:00Z".into()),
            pending_changes: 5,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("5"));
        assert!(json.contains("2024-01-01"));
    }

    #[test]
    fn test_sync_status_response_none_time() {
        let status = SyncStatusResponse {
            last_sync_time: None,
            pending_changes: 0,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("null"));
        assert!(json.contains("0"));
    }

    #[test]
    fn test_sync_pull_params_deserialization() {
        let json = r#"{"local_version": 100}"#;
        let params: SyncPullParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.local_version, Some(100));
    }

    #[test]
    fn test_sync_pull_params_deserialization_empty() {
        let json = r#"{}"#;
        let params: SyncPullParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.local_version, None);
    }
}
