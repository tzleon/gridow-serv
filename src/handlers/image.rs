//! 图片管理处理器
//!
//! 提供图片上传和访问功能：
//! * 上传支持 JPG / PNG / WEBP 格式，限制 10MB
//! * 文件名使用 UUID v4 生成，避免冲突
//! * 通过 `Content-Type` 头正确返回 MIME 类型
//!
//! # 安全说明
//! 当前为公开接口（无需认证），适用于物品图片等非敏感资源。

use axum::extract::{Multipart, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::models::error::AppError;
use crate::state::AppState;

/// 上传图片
///
/// 接收 `multipart/form-data` 格式的单个文件（字段名 `file`）。
/// 返回图片 ID 和访问 URL。
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
            // 获取 Content-Type
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

            // 限制 10MB
            if data.len() > 10 * 1024 * 1024 {
                return Err(AppError::PayloadTooLarge);
            }

            // 根据 Content-Type 确定扩展名
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

/// 获取图片
///
/// 根据文件名（含扩展名）返回图片二进制数据，
/// 设置正确的 `Content-Type` 头。
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

    // 根据扩展名推断 MIME 类型
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

/// 检查是否为允许的图片 Content-Type
fn is_allowed_content_type(ct: &str) -> bool {
    matches!(ct, "image/jpeg" | "image/png" | "image/webp")
}
