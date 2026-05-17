//! 图片管理处理器
//!
//! 提供图片上传和访问功能：
//! * 上传支持 JPG / PNG / WEBP 格式，限制 10MB
//! * 自动检查图片大小，超过阈值自动缩放至缩略图（最大 800x800 像素）
//! * 文件名使用 UUID v4 生成，避免冲突
//! * 通过 Content-Type 头正确返回 MIME 类型
//!
//! # 安全说明
//! 当前为公开接口（无需认证），适用于物品图片等非敏感资源。

use axum::extract::{Multipart, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use image::ImageFormat;
use std::io::Cursor;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::models::error::AppError;
use crate::state::AppState;

/// 缩略图最大尺寸
const MAX_THUMBNAIL_SIZE: u32 = 800;

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

            // 处理图片，超过阈值自动缩放到缩略图大小
            let (processed_data, output_ext) = process_image(&data, &content_type)?;

            image_id = Uuid::new_v4().to_string();
            let filename = format!("{}.{}", image_id, output_ext);
            let filepath = std::path::Path::new(&state.upload_dir).join(&filename);

            let mut file = tokio::fs::File::create(&filepath)
                .await
                .map_err(|e| AppError::Internal(format!("创建文件失败: {}", e)))?;

            file.write_all(&processed_data)
                .await
                .map_err(|e| AppError::Internal(format!("写入文件失败: {}", e)))?;

            image_url = format!("{}/v1/images/{}", state.base_url.trim_end_matches('/'), filename);
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

/// 处理图片：检查大小并在超过阈值时自动缩放
///
/// 最大尺寸为 800x800 像素，保持宽高比
fn process_image(data: &[u8], content_type: &str) -> Result<(Vec<u8>, String), AppError> {
    let format = match content_type {
        "image/jpeg" => ImageFormat::Jpeg,
        "image/png" => ImageFormat::Png,
        "image/webp" => ImageFormat::WebP,
        _ => return Err(AppError::BadRequest("不支持的图片格式".to_string())),
    };

    // 从字节数据加载图片
    let img = image::load_from_memory_with_format(data, format)
        .map_err(|e| AppError::BadRequest(format!("图片加载失败: {}", e)))?;

    // 检查是否需要缩放
    if img.width() <= MAX_THUMBNAIL_SIZE && img.height() <= MAX_THUMBNAIL_SIZE {
        // 图片已经符合尺寸要求，直接返回原数据
        Ok((data.to_vec(), content_type_to_ext(content_type)))
    } else {
        // 缩放到缩略图大小，保持宽高比
        let resized_img = img.resize(MAX_THUMBNAIL_SIZE, MAX_THUMBNAIL_SIZE, image::imageops::FilterType::Lanczos3);

        // 将处理后的图片写回到字节数组
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        // 保存为 JPEG 格式，质量 85（平衡质量和文件大小）
        resized_img
            .write_to(&mut cursor, ImageFormat::Jpeg)
            .map_err(|e| AppError::Internal(format!("图片处理失败: {}", e)))?;

        Ok((buffer, "jpg".to_string()))
    }
}

/// 将 Content-Type 转换为文件扩展名
fn content_type_to_ext(content_type: &str) -> String {
    match content_type {
        "image/jpeg" => "jpg".to_string(),
        "image/png" => "png".to_string(),
        "image/webp" => "webp".to_string(),
        _ => "bin".to_string(),
    }
}

/// 获取图片
///
/// 根据文件名（含扩展名）返回图片二进制数据，
/// 设置正确的 Content-Type 头。
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
