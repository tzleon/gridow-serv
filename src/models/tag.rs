use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Default)]
pub struct Tag {
    #[serde(skip)]
    pub id: i64,
    #[serde(rename = "id")]
    pub public_id: String,
    pub name: String,
    #[serde(skip)]
    pub owner_id: i64,
    pub created_at: String,
    pub version: i64,
    #[serde(rename = "is_deleted")]
    pub is_deleted: i16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncTagChange {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<Tag>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub updated: Vec<Tag>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deleted: Vec<String>,
}

impl SyncTagChange {
    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.updated.is_empty() && self.deleted.is_empty()
    }
    pub fn opt(self) -> Option<Self> {
        if self.is_empty() { None } else { Some(self) }
    }
}

#[derive(Debug, Deserialize)]
pub struct TagCreateRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct TagUpdateRequest {
    pub name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── SyncTagChange ────────────────────────────────────

    #[test]
    fn test_sync_tag_change_is_empty_all_empty() {
        let change = SyncTagChange {
            created: vec![],
            updated: vec![],
            deleted: vec![],
        };
        assert!(change.is_empty());
    }

    #[test]
    fn test_sync_tag_change_is_empty_with_created() {
        let change = SyncTagChange {
            created: vec![Tag { public_id: "t1".into(), name: "tag1".into(), ..Default::default() }],
            updated: vec![],
            deleted: vec![],
        };
        assert!(!change.is_empty());
    }

    #[test]
    fn test_sync_tag_change_is_empty_with_deleted() {
        let change = SyncTagChange {
            created: vec![],
            updated: vec![],
            deleted: vec!["t1".into()],
        };
        assert!(!change.is_empty());
    }

    #[test]
    fn test_sync_tag_change_opt_returns_none_when_empty() {
        let change = SyncTagChange {
            created: vec![],
            updated: vec![],
            deleted: vec![],
        };
        assert!(change.opt().is_none());
    }

    #[test]
    fn test_sync_tag_change_opt_returns_some_when_not_empty() {
        let change = SyncTagChange {
            created: vec![],
            updated: vec![],
            deleted: vec!["t1".into()],
        };
        assert!(change.opt().is_some());
    }

    // ── Tag 序列化 ───────────────────────────────────────

    #[test]
    fn test_tag_serialization_skips_internal_fields() {
        let tag = Tag {
            id: 123, public_id: "pub_t1".into(), name: "test".into(),
            owner_id: 999, created_at: "now".into(), version: 1, is_deleted: 0,
        };
        let json = serde_json::to_string(&tag).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["id"], "pub_t1");
        assert!(parsed.get("owner_id").is_none());
    }

    #[test]
    fn test_tag_create_request() {
        let json = r#"{"name": "重要"}"#;
        let req: TagCreateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "重要");
    }

    #[test]
    fn test_tag_update_request_full() {
        let json = r#"{"name": "新名称"}"#;
        let req: TagUpdateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("新名称".into()));
    }

    #[test]
    fn test_tag_update_request_partial() {
        let json = r#"{}"#;
        let req: TagUpdateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, None);
    }

    #[test]
    fn test_tag_create_request_missing_name() {
        let json = r#"{}"#;
        let result = serde_json::from_str::<TagCreateRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_tag_change_with_updated() {
        let change = SyncTagChange {
            created: vec![],
            updated: vec![Tag { public_id: "t1".into(), name: "tag1".into(), ..Default::default() }],
            deleted: vec![],
        };
        assert!(!change.is_empty());
    }
}
