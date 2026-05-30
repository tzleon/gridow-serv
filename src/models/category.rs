use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Default)]
pub struct Category {
    #[serde(skip)]
    pub id: i64,
    #[serde(rename = "id")]
    pub public_id: String,
    pub name: String,
    pub icon: String,
    pub sort_order: i32,
    #[serde(skip)]
    pub owner_id: i64,
    pub created_at: String,
    #[sqlx(default)]
    #[serde(default)]
    pub item_count: i64,
    #[sqlx(default)]
    #[serde(default)]
    pub last_used_at: Option<String>,
    pub version: i64,
    #[serde(rename = "is_deleted")]
    pub is_deleted: i16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncCategoryChange {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<Category>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub updated: Vec<Category>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deleted: Vec<String>,
}

impl SyncCategoryChange {
    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.updated.is_empty() && self.deleted.is_empty()
    }
    pub fn opt(self) -> Option<Self> {
        if self.is_empty() { None } else { Some(self) }
    }
}

#[derive(Debug, Deserialize)]
pub struct CategoryCreateRequest {
    pub name: String,
    #[serde(default = "default_icon")]
    pub icon: String,
}

#[derive(Debug, Deserialize)]
pub struct CategoryUpdateRequest {
    pub name: Option<String>,
    pub icon: Option<String>,
}

fn default_icon() -> String { "📦".to_string() }

#[cfg(test)]
mod tests {
    use super::*;

    // ── SyncCategoryChange ───────────────────────────────

    #[test]
    fn test_sync_category_change_is_empty_all_empty() {
        let change = SyncCategoryChange {
            created: vec![],
            updated: vec![],
            deleted: vec![],
        };
        assert!(change.is_empty());
    }

    #[test]
    fn test_sync_category_change_is_empty_with_created() {
        let change = SyncCategoryChange {
            created: vec![Category { public_id: "c1".into(), name: "cat1".into(), ..Default::default() }],
            updated: vec![],
            deleted: vec![],
        };
        assert!(!change.is_empty());
    }

    #[test]
    fn test_sync_category_change_is_empty_with_deleted() {
        let change = SyncCategoryChange {
            created: vec![],
            updated: vec![],
            deleted: vec!["c1".into()],
        };
        assert!(!change.is_empty());
    }

    #[test]
    fn test_sync_category_change_opt_returns_none_when_empty() {
        let change = SyncCategoryChange {
            created: vec![],
            updated: vec![],
            deleted: vec![],
        };
        assert!(change.opt().is_none());
    }

    #[test]
    fn test_sync_category_change_opt_returns_some_when_not_empty() {
        let change = SyncCategoryChange {
            created: vec![],
            updated: vec![],
            deleted: vec!["c1".into()],
        };
        assert!(change.opt().is_some());
    }

    // ── Category 序列化 ──────────────────────────────────

    #[test]
    fn test_category_serialization_skips_internal_id() {
        let cat = Category {
            id: 12345, public_id: "pub_abc".into(), name: "test".into(), icon: "📦".into(),
            sort_order: 1, owner_id: 999, created_at: "now".into(),
            item_count: 5, last_used_at: Some("now".into()), version: 1, is_deleted: 0,
        };
        let json = serde_json::to_string(&cat).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        // id 应序列化为 public_id 的值
        assert_eq!(parsed["id"], "pub_abc");
        // owner_id 内部字段应被跳过
        assert!(parsed.get("owner_id").is_none());
    }

    #[test]
    fn test_category_deserialization_from_json() {
        let json = r#"{
            "name": "电子设备",
            "icon": "💻"
        }"#;
        let req: CategoryCreateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "电子设备");
        assert_eq!(req.icon, "💻");
    }

    #[test]
    fn test_category_create_request_default_icon() {
        let json = r#"{"name": "默认图标"}"#;
        let req: CategoryCreateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "默认图标");
        assert_eq!(req.icon, "📦");
    }

    #[test]
    fn test_category_update_request_partial() {
        let json = r#"{"name": "新名称"}"#;
        let req: CategoryUpdateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("新名称".into()));
        assert_eq!(req.icon, None);
    }

    #[test]
    fn test_category_create_request_missing_name() {
        let json = r#"{"icon": "📦"}"#;
        let result = serde_json::from_str::<CategoryCreateRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_category_update_request_empty() {
        let json = r#"{}"#;
        let req: CategoryUpdateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, None);
        assert_eq!(req.icon, None);
    }

    #[test]
    fn test_sync_category_change_with_updated() {
        let change = SyncCategoryChange {
            created: vec![],
            updated: vec![Category { public_id: "c1".into(), name: "cat1".into(), ..Default::default() }],
            deleted: vec![],
        };
        assert!(!change.is_empty());
    }
}
