use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Default)]
pub struct Space {
    #[serde(skip)]
    pub id: i64,
    #[serde(rename = "id")]
    pub public_id: String,
    pub name: String,
    pub icon: String,
    pub count: i32,
    #[serde(skip)]
    pub parent_id: Option<i64>,
    #[serde(rename = "parent_id")]
    #[sqlx(default)]
    pub parent_public_id: Option<String>,
    pub depth: i32,
    pub sort_order: i32,
    pub photo_uri: String,
    #[serde(skip)]
    pub owner_id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
    #[serde(rename = "is_deleted")]
    pub is_deleted: i16,
}

#[derive(Debug, Deserialize)]
pub struct SpaceCreateRequest {
    pub name: String,
    #[serde(default = "default_space_icon")]
    pub icon: String,
    pub parent_id: Option<String>,
    #[serde(default)]
    pub photo_uri: String,
}

#[derive(Debug, Deserialize)]
pub struct SpaceUpdateRequest {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub photo_uri: Option<String>,
    pub sort_order: Option<i32>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceNode {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub count: i32,
    pub parent_id: Option<String>,
    pub depth: i32,
    pub photo_uri: String,
    pub children: Vec<SpaceNode>,
    pub item_ids: Vec<String>,
    pub owner_id: String,
    pub version: i64,
    #[serde(rename = "is_deleted")]
    pub is_deleted: i16,
}

#[derive(Debug, Serialize)]
pub struct SpacePathSegment {
    pub id: String,
    pub name: String,
    pub icon: String,
}

#[derive(Debug, Serialize)]
pub struct SpacePathResponse {
    pub path: String,
    pub segments: Vec<SpacePathSegment>,
}

fn default_space_icon() -> String { "🏠".to_string() }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_space_serialization_skips_internal_fields() {
        let space = Space {
            id: 12345, public_id: "pub_s1".into(), name: "room".into(), icon: "🏠".into(),
            count: 5, parent_id: Some(1), parent_public_id: Some("pub_p1".into()),
            depth: 1, sort_order: 0, photo_uri: "".into(), owner_id: 999,
            created_at: "now".into(), updated_at: "now".into(), version: 2, is_deleted: 0,
        };
        let json = serde_json::to_string(&space).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["id"], "pub_s1");
        assert_eq!(parsed["parent_id"], "pub_p1");
        // 内部字段应被跳过
        assert!(parsed.get("owner_id").is_none(), "owner_id should be skipped");
    }

    #[test]
    fn test_space_create_request() {
        let json = r#"{"name": "新空间", "parent_id": "parent123"}"#;
        let req: SpaceCreateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "新空间");
        assert_eq!(req.parent_id, Some("parent123".into()));
        assert_eq!(req.icon, "🏠");
        assert_eq!(req.photo_uri, "");
    }

    #[test]
    fn test_space_create_request_defaults() {
        let json = r#"{"name": "根空间"}"#;
        let req: SpaceCreateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "根空间");
        assert_eq!(req.parent_id, None);
        assert_eq!(req.icon, "🏠");
    }

    #[test]
    fn test_space_update_request_partial() {
        let json = r#"{"name": "新名称"}"#;
        let req: SpaceUpdateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("新名称".into()));
        assert_eq!(req.icon, None);
        assert_eq!(req.parent_id, None);
    }

    #[test]
    fn test_space_update_request_full() {
        let json = r#"{"name": "new", "icon": "📁", "photo_uri": "photo.jpg", "sort_order": 1, "parent_id": "p1"}"#;
        let req: SpaceUpdateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("new".into()));
        assert_eq!(req.icon, Some("📁".into()));
        assert_eq!(req.photo_uri, Some("photo.jpg".into()));
        assert_eq!(req.sort_order, Some(1));
        assert_eq!(req.parent_id, Some("p1".into()));
    }

    #[test]
    fn test_space_node_serialization() {
        let node = SpaceNode {
            id: "s1".into(), name: "root".into(), icon: "🏠".into(), count: 0,
            parent_id: None, depth: 0, photo_uri: "".into(),
            children: vec![SpaceNode {
                id: "s2".into(), name: "child".into(), icon: "📁".into(), count: 0,
                parent_id: Some("s1".into()), depth: 1, photo_uri: "".into(),
                children: vec![], item_ids: vec![], owner_id: "u1".into(),
                version: 0, is_deleted: 0,
            }],
            item_ids: vec![], owner_id: "u1".into(), version: 0, is_deleted: 0,
        };
        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("root"));
        assert!(json.contains("child"));
        assert!(json.contains("s2"));
    }

    #[test]
    fn test_space_path_response_serialization() {
        let resp = SpacePathResponse {
            path: "/root/child".into(),
            segments: vec![
                SpacePathSegment { id: "s1".into(), name: "root".into(), icon: "🏠".into() },
                SpacePathSegment { id: "s2".into(), name: "child".into(), icon: "📁".into() },
            ],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("/root/child"));
        assert!(json.contains("s1"));
        assert!(json.contains("s2"));
    }

    #[test]
    fn test_space_path_segment_serialization() {
        let seg = SpacePathSegment { id: "s1".into(), name: "root".into(), icon: "🏠".into() };
        let json = serde_json::to_string(&seg).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "s1");
        assert_eq!(parsed["name"], "root");
    }

    #[test]
    fn test_space_create_request_missing_name() {
        let json = r#"{"icon": "🏠"}"#;
        let result = serde_json::from_str::<SpaceCreateRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_space_update_request_empty() {
        let json = r#"{}"#;
        let req: SpaceUpdateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, None);
        assert_eq!(req.icon, None);
        assert_eq!(req.photo_uri, None);
        assert_eq!(req.sort_order, None);
        assert_eq!(req.parent_id, None);
    }

    #[test]
    fn test_space_serialization_skips_internal_id() {
        let space = Space {
            id: 999, public_id: "pub_s1".into(), name: "room".into(), icon: "🏠".into(),
            count: 0, parent_id: None, parent_public_id: None,
            depth: 0, sort_order: 0, photo_uri: "".into(), owner_id: 888,
            created_at: "now".into(), updated_at: "now".into(), version: 0, is_deleted: 0,
        };
        let json = serde_json::to_string(&space).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("id").is_none() || parsed["id"] == "pub_s1", "internal id should not appear");
    }
}
