use gridow_web::models::category::{Category, CategoryCreateRequest, SyncCategoryChange};
use gridow_web::models::history::HistoryRecord;
use gridow_web::models::item::Item;
use gridow_web::models::space::Space;
use gridow_web::models::sync::{
    IdVersionMapping, SyncConflict, SyncEntityChange, SyncHistoryChange, SyncPullResponse,
    SyncPushResponse, SyncStatusResponse,
};
use gridow_web::models::tag::SyncTagChange;
use gridow_web::models::user::UserInfo;

// ── 跨模块组合：SyncPullResponse 包含多种实体 ──────────────

#[test]
fn test_sync_pull_response_composition() {
    let item = Item {
        public_id: "item1".into(), name: "矿泉水".into(), icon: "💧".into(),
        qty: 24, location: "客厅".into(), category: "food".into(),
        created_at: "now".into(), updated_at: "now".into(), ..Default::default()
    };
    let space = Space {
        public_id: "space1".into(), name: "客厅".into(), icon: "🏠".into(),
        count: 1, created_at: "now".into(), updated_at: "now".into(), ..Default::default()
    };
    let history = HistoryRecord {
        public_id: "h1".into(), r#type: "in".into(),
        item_public_id: "item1".into(), item_name: "矿泉水".into(),
        qty: 24, to_location: Some("客厅".into()), time: "now".into(), ..Default::default()
    };

    let resp = SyncPullResponse {
        items: Some(SyncEntityChange {
            created: vec![item], updated: vec![], deleted: vec![],
        }),
        spaces: Some(SyncEntityChange {
            created: vec![space], updated: vec![], deleted: vec![],
        }),
        history: Some(SyncHistoryChange {
            created: vec![history], deleted: vec![],
        }),
        categories: None,
        tags: None,
        has_more: false,
    };

    let json = serde_json::to_string(&resp).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert!(parsed["items"]["created"].is_array());
    assert!(parsed["spaces"]["created"].is_array());
    assert!(parsed["history"]["created"].is_array());
    assert!(parsed.get("categories").is_none(), "None fields should be skipped");
    assert!(parsed.get("tags").is_none(), "None fields should be skipped");
    assert_eq!(parsed["has_more"], false);
}

#[test]
fn test_sync_pull_response_all_empty() {
    let resp = SyncPullResponse {
        items: None, spaces: None, history: None,
        categories: None, tags: None, has_more: false,
    };
    let json = serde_json::to_string(&resp).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["has_more"], false);
    assert!(parsed.get("items").is_none());
}

#[test]
fn test_sync_push_response_with_conflicts_and_mappings() {
    let resp = SyncPushResponse {
        success: false,
        conflicts: vec![SyncConflict {
            entity: "item".into(),
            id: "item1".into(),
            reason: "version mismatch".into(),
        }],
        assigned_items: vec![IdVersionMapping {
            client_id: "c_item1".into(),
            server_id: "s_item1".into(),
            version: 42,
        }],
        assigned_spaces: vec![],
        assigned_history: vec![],
        assigned_categories: vec![],
        assigned_tags: vec![],
    };
    let json = serde_json::to_string(&resp).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed["success"], false);
    assert!(parsed["conflicts"].is_array());
    assert_eq!(parsed["conflicts"][0]["entity"], "item");
    assert!(parsed["assigned_items"].is_array());
    assert!(parsed.get("assigned_spaces").is_none(), "empty vec should be skipped");
}

// ── 跨模块 Sync 变体一致性 ──────────────────────────────

#[test]
fn test_all_sync_change_variants_consistent_behavior() {
    let empty_item: SyncEntityChange<Item> = SyncEntityChange {
        created: vec![], updated: vec![], deleted: vec![],
    };
    let empty_cat = SyncCategoryChange { created: vec![], updated: vec![], deleted: vec![] };
    let empty_tag = SyncTagChange { created: vec![], updated: vec![], deleted: vec![] };
    let empty_history = SyncHistoryChange { created: vec![], deleted: vec![] };

    assert!(empty_item.is_empty());
    assert!(empty_cat.is_empty());
    assert!(empty_tag.is_empty());
    assert!(empty_history.is_empty());

    assert!(empty_item.opt().is_none());
    assert!(empty_cat.opt().is_none());
    assert!(empty_tag.opt().is_none());
    assert!(empty_history.opt().is_none());
}

#[test]
fn test_sync_entity_change_with_deleted_only() {
    let change: SyncEntityChange<String> = SyncEntityChange {
        created: vec![], updated: vec![], deleted: vec!["id1".into()],
    };
    assert!(!change.is_empty());
    assert!(change.opt().is_some());
}

// ── 公共 API 可达性（验证 lib crate 导出路径）──────────────

#[test]
fn test_public_api_category_create_accessible() {
    let req: CategoryCreateRequest = serde_json::from_str(r#"{"name": "测试"}"#).unwrap();
    assert_eq!(req.name, "测试");
    assert_eq!(req.icon, "📦");
}

#[test]
fn test_public_api_sync_types_accessible() {
    let status = SyncStatusResponse {
        last_sync_time: None,
        pending_changes: 0,
    };
    let json = serde_json::to_string(&status).unwrap();
    assert!(json.contains("null"));
}

#[test]
fn test_public_api_id_version_mapping_roundtrip() {
    let mapping = IdVersionMapping {
        client_id: "c1".into(), server_id: "s1".into(), version: 42,
    };
    let json = serde_json::to_string(&mapping).unwrap();
    let restored: IdVersionMapping = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.client_id, "c1");
    assert_eq!(restored.server_id, "s1");
    assert_eq!(restored.version, 42);
}

#[test]
fn test_public_api_sync_conflict_roundtrip() {
    let conflict = SyncConflict {
        entity: "item".into(), id: "abc".into(), reason: "version conflict".into(),
    };
    let json = serde_json::to_string(&conflict).unwrap();
    let restored: SyncConflict = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.entity, "item");
    assert_eq!(restored.id, "abc");
    assert_eq!(restored.reason, "version conflict");
}

// ── 跨模块 serde round-trip ──────────────────────────────

#[test]
fn test_user_info_roundtrip_via_public_api() {
    let original = UserInfo {
        id: "u1".into(), username: "test".into(), email: "t@t.com".into(),
        avatar: "av.jpg".into(), role: "user".into(), status: "active".into(),
        created_at: "2024-01-01".into(),
    };
    let json = serde_json::to_string(&original).unwrap();
    let restored: UserInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(restored.id, original.id);
    assert_eq!(restored.username, original.username);
    assert_eq!(restored.email, original.email);
    assert_eq!(restored.role, original.role);
}
