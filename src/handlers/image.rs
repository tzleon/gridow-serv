use axum::extract::{Multipart, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::models::error::AppError;
use crate::state::AppState;

pub async fn upload_image(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let mut image_id = String::new();
    let mut image_url = String::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("上传失败: {}", e)))?
    {
        let field_name = field.name().unwrap_or("").to_string();

        if field_name == "file" {
            let content_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();

            if !is_allowed_content_type(&content_type) {
                return Err(AppError::BadRequest(
                    "仅支持 JPG/PNG/WEBP 格式".to_string(),
                ));
            }

            let data = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(format!("读取文件失败: {}", e)))?;

            if data.len() > 10 * 1024 * 1024 {
                return Err(AppError::PayloadTooLarge);
            }

            let ext = match content_type.as_str() {
                "image/jpeg" => "jpg",
                "image/png" => "png",
                "image/webp" => "webp",
                _ => "bin",
            };

            image_id = Uuid::new_v4().to_string();
            let filename = format!("{}.{}", image_id, ext);
            let filepath = std::path::Path::new(&state.upload_dir).join(&filename);

            let mut file = tokio::fs::File::create(&filepath)
                .await
                .map_err(|e| AppError::Internal(format!("创建文件失败: {}", e)))?;

            file.write_all(&data)
                .await
                .map_err(|e| AppError::Internal(format!("写入文件失败: {}", e)))?;

            image_url = format!("/v1/images/{}", filename);
        }
    }

    if image_id.is_empty() {
        return Err(AppError::BadRequest("未提供文件".to_string()));
    }

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": image_id,
            "url": image_url,
        })),
    ))
}

pub async fn get_image(
    State(state): State<AppState>,
    Path(filename): Path<String>,
) -> Result<Response, AppError> {
    let filepath = std::path::Path::new(&state.upload_dir).join(&filename);

    if !filepath.exists() {
        return Err(AppError::NotFound);
    }

    let data = tokio::fs::read(&filepath)
        .await
        .map_err(|e| AppError::Internal(format!("读取文件失败: {}", e)))?;

    let content_type = match filepath.extension().and_then(|e| e.to_str()) {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    };

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, content_type)],
        data,
    )
        .into_response())
}

fn is_allowed_content_type(ct: &str) -> bool {
    matches!(ct, "image/jpeg" | "image/png" | "image/webp")
}
