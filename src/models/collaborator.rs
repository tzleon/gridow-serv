use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Collaborator {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub user_id: String,
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
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub avatar: String,
}
