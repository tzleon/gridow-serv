use axum::extract::{Path, State};
use axum::Json;

use crate::auth::AuthUser;
use crate::models::collaborator::*;
use crate::models::error::AppError;
use crate::state::AppState;

async fn resolve_entity_owner(state: &AppState, entity_type: &str, public_id: &str) -> Result<(i64, i64), AppError> {
    match entity_type {
        "item" => {
            let row: (i64, i64) = sqlx::query_as("SELECT id, owner_id FROM items WHERE public_id = $1")
                .bind(public_id).fetch_optional(&state.db).await
                .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;
            Ok(row)
        }
        "space" => {
            let row: (i64, i64) = sqlx::query_as("SELECT id, owner_id FROM spaces WHERE public_id = $1")
                .bind(public_id).fetch_optional(&state.db).await
                .map_err(AppError::Database)?.ok_or(AppError::NotFound)?;
            Ok(row)
        }
        _ => Err(AppError::BadRequest("无效的实体类型".to_string())),
    }
}

async fn add_collaborator_inner(
    state: &AppState, auth: &AuthUser, entity_type: &str,
    entity_public_id: &str, target_user_public_id: &str,
) -> Result<Collaborator, AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let (entity_internal, owner_id) = resolve_entity_owner(state, entity_type, entity_public_id).await?;
    if owner_id != user_internal { return Err(AppError::Forbidden); }

    let target_internal = state.resolve_user_id(target_user_public_id).await?;
    let (id, public_id) = state.new_id();
    let now = AppState::now_string();

    let collaborator = sqlx::query_as::<_, Collaborator>(
        r#"INSERT INTO collaborators (id, public_id, entity_type, entity_id, user_id, created_at)
           VALUES ($1, $2, $3, $4, $5, $6)
           ON CONFLICT (entity_type, entity_id, user_id) DO UPDATE SET user_id = $5
           RETURNING *"#,
    ).bind(id).bind(&public_id).bind(entity_type).bind(entity_internal).bind(target_internal).bind(&now)
    .fetch_one(&state.db).await.map_err(AppError::Database)?;

    Ok(collaborator)
}

async fn remove_collaborator_inner(
    state: &AppState, auth: &AuthUser, entity_type: &str,
    entity_public_id: &str, target_user_public_id: &str,
) -> Result<(), AppError> {
    let user_internal = state.resolve_user_id(&auth.public_id).await?;
    let (entity_internal, owner_id) = resolve_entity_owner(state, entity_type, entity_public_id).await?;
    if owner_id != user_internal { return Err(AppError::Forbidden); }

    let target_internal = state.resolve_user_id(target_user_public_id).await?;

    sqlx::query("DELETE FROM collaborators WHERE entity_type = $1 AND entity_id = $2 AND user_id = $3")
        .bind(entity_type).bind(entity_internal).bind(target_internal)
        .execute(&state.db).await.map_err(AppError::Database)?;

    Ok(())
}

async fn list_collaborators_inner(
    state: &AppState, entity_type: &str, entity_public_id: &str,
) -> Result<Vec<CollaboratorInfo>, AppError> {
    let (entity_internal, _) = resolve_entity_owner(state, entity_type, entity_public_id).await?;

    let collaborators = sqlx::query_as::<_, CollaboratorInfo>(
        r#"SELECT c.id, c.user_id, u.username, u.email, u.avatar
           FROM collaborators c JOIN users u ON c.user_id = u.id
           WHERE c.entity_type = $1 AND c.entity_id = $2 ORDER BY c.created_at"#,
    ).bind(entity_type).bind(entity_internal).fetch_all(&state.db).await.map_err(AppError::Database)?;

    Ok(collaborators)
}

pub async fn add_item_collaborator(
    State(state): State<AppState>, auth: AuthUser, Path(item_public_id): Path<String>,
    Json(req): Json<AddCollaboratorRequest>,
) -> Result<Json<Collaborator>, AppError> {
    let c = add_collaborator_inner(&state, &auth, "item", &item_public_id, &req.user_id).await?;
    Ok(Json(c))
}

pub async fn remove_item_collaborator(
    State(state): State<AppState>, auth: AuthUser,
    Path((item_public_id, user_public_id)): Path<(String, String)>,
) -> Result<axum::http::StatusCode, AppError> {
    remove_collaborator_inner(&state, &auth, "item", &item_public_id, &user_public_id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn list_item_collaborators(
    State(state): State<AppState>, auth: AuthUser, Path(item_public_id): Path<String>,
) -> Result<Json<CollaboratorListResponse>, AppError> {
    let _user_internal = state.resolve_user_id(&auth.public_id).await?;
    let collaborators = list_collaborators_inner(&state, "item", &item_public_id).await?;
    Ok(Json(CollaboratorListResponse { collaborators }))
}

pub async fn add_space_collaborator(
    State(state): State<AppState>, auth: AuthUser, Path(space_public_id): Path<String>,
    Json(req): Json<AddCollaboratorRequest>,
) -> Result<Json<Collaborator>, AppError> {
    let c = add_collaborator_inner(&state, &auth, "space", &space_public_id, &req.user_id).await?;
    Ok(Json(c))
}

pub async fn remove_space_collaborator(
    State(state): State<AppState>, auth: AuthUser,
    Path((space_public_id, user_public_id)): Path<(String, String)>,
) -> Result<axum::http::StatusCode, AppError> {
    remove_collaborator_inner(&state, &auth, "space", &space_public_id, &user_public_id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn list_space_collaborators(
    State(state): State<AppState>, auth: AuthUser, Path(space_public_id): Path<String>,
) -> Result<Json<CollaboratorListResponse>, AppError> {
    let _user_internal = state.resolve_user_id(&auth.public_id).await?;
    let collaborators = list_collaborators_inner(&state, "space", &space_public_id).await?;
    Ok(Json(CollaboratorListResponse { collaborators }))
}
