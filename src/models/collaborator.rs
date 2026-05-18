use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CollaboratorInfo {
    #[serde(skip)]
    pub id: i64,
    #[serde(skip)]
    pub user_id: i64,
    pub username: String,
    pub email: String,
    pub avatar: String,
}
