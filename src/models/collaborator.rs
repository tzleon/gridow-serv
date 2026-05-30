use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Default)]
pub struct Collaborator {
    #[serde(skip)]
    pub id: i64,
    #[serde(rename = "id")]
    pub public_id: String,
    pub entity_type: String,
    #[serde(skip)]
    pub entity_id: i64,
    #[serde(skip)]
    pub user_id: i64,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct AddCollaboratorRequest {
    pub user_id: String,
}

#[derive(Debug, Serialize)]
pub struct CollaboratorListResponse {
    pub collaborators: Vec<CollaboratorInfo>,
}

#[derive(Debug, Serialize, sqlx::FromRow, Default)]
pub struct CollaboratorInfo {
    #[serde(skip)]
    pub id: i64,
    #[serde(skip)]
    pub user_id: i64,
    pub username: String,
    pub email: String,
    pub avatar: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collaborator_serialization() {
        let collab = Collaborator {
            id: 123, public_id: "c1".into(), entity_type: "item".into(),
            entity_id: 456, user_id: 789, created_at: "now".into(),
        };
        let json = serde_json::to_string(&collab).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["id"], "c1");
        assert_eq!(parsed["entity_type"], "item");
        // 内部字段应被跳过
        assert!(parsed.get("entity_id").is_none());
        assert!(parsed.get("user_id").is_none());
    }

    #[test]
    fn test_add_collaborator_request() {
        let json = r#"{"user_id": "user123"}"#;
        let req: AddCollaboratorRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.user_id, "user123");
    }

    #[test]
    fn test_collaborator_list_response_serialization() {
        let resp = CollaboratorListResponse {
            collaborators: vec![
                CollaboratorInfo {
                    id: 1, user_id: 100, username: "user1".into(),
                    email: "u1@test.com".into(), avatar: "av1.jpg".into(),
                },
                CollaboratorInfo {
                    id: 2, user_id: 200, username: "user2".into(),
                    email: "u2@test.com".into(), avatar: "av2.jpg".into(),
                },
            ],
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("user1"));
        assert!(json.contains("user2"));
        assert!(json.contains("u1@test.com"));
    }

    #[test]
    fn test_collaborator_info_serialization_skips_internal_fields() {
        let info = CollaboratorInfo {
            id: 123, user_id: 456, username: "test".into(),
            email: "test@test.com".into(), avatar: "av.jpg".into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(parsed.get("id").is_none(), "id should be skipped");
        assert!(parsed.get("user_id").is_none(), "user_id should be skipped");
        assert_eq!(parsed["username"], "test");
    }
}
