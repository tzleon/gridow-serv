use axum::extract::{Multipart, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use image::ImageFormat;
use std::io::Cursor;
use tokio::io::AsyncWriteExt;

use crate::auth::AuthUser;
use crate::models::error::AppError;
use crate::state::AppState;

const MAX_THUMBNAIL_SIZE: u32 = 800;
const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

pub async fn upload_image(
    State(state): State<AppState>,
    auth: AuthUser,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let _user_internal = state.resolve_user_id(&auth.public_id).await?;

    let mut upload_type = String::new();
    let mut requested_filename: Option<String> = None;
    let mut content_type = String::new();
    let mut file_data: Option<Vec<u8>> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("上传失败: {}", e)))?
    {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "type" => {
                upload_type = field
                    .text()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("读取类型失败: {}", e)))?
                    .trim()
                    .to_lowercase();
            }
            "file" => {
                requested_filename = field.file_name().map(|name| name.to_string());
                content_type = field
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

                if data.len() > MAX_FILE_SIZE {
                    return Err(AppError::PayloadTooLarge);
                }

                file_data = Some(data.to_vec());
            }
            _ => {}
        }
    }

    let file_data = file_data.ok_or_else(|| AppError::BadRequest("未提供文件".to_string()))?;
    let (processed_data, output_ext) = process_image(&file_data, &content_type)?;
    let image_id = requested_filename
        .as_deref()
        .filter(|_| upload_type == "avatar")
        .and_then(extract_avatar_filename_stem)
        .unwrap_or_else(|| state.new_public_id());
    let filename = format!("{}.{}", image_id, output_ext);
    let filepath = std::path::Path::new(&state.upload_dir).join(&filename);

    if filepath.exists() {
        return Err(AppError::BadRequest("文件名已存在，请重新上传".to_string()));
    }

    let mut file = tokio::fs::File::create(&filepath)
        .await
        .map_err(|e| AppError::Internal(format!("创建文件失败: {}", e)))?;

    file.write_all(&processed_data)
        .await
        .map_err(|e| AppError::Internal(format!("写入文件失败: {}", e)))?;

    let image_url = format!("{}/v1/images/{}", state.base_url.trim_end_matches('/'), filename);

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": image_id,
            "url": image_url,
        })),
    ))
}

fn process_image(data: &[u8], content_type: &str) -> Result<(Vec<u8>, String), AppError> {
    let format = match content_type {
        "image/jpeg" => ImageFormat::Jpeg,
        "image/png" => ImageFormat::Png,
        "image/webp" => ImageFormat::WebP,
        _ => return Err(AppError::BadRequest("不支持的图片格式".to_string())),
    };

    let img = image::load_from_memory_with_format(data, format)
        .map_err(|e| AppError::BadRequest(format!("图片加载失败: {}", e)))?;

    if img.width() <= MAX_THUMBNAIL_SIZE && img.height() <= MAX_THUMBNAIL_SIZE {
        Ok((data.to_vec(), content_type_to_ext(content_type)))
    } else {
        let resized_img = img.resize(MAX_THUMBNAIL_SIZE, MAX_THUMBNAIL_SIZE, image::imageops::FilterType::Lanczos3);

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        resized_img
            .write_to(&mut cursor, ImageFormat::Jpeg)
            .map_err(|e| AppError::Internal(format!("图片处理失败: {}", e)))?;

        Ok((buffer, "jpg".to_string()))
    }
}

fn content_type_to_ext(content_type: &str) -> String {
    match content_type {
        "image/jpeg" => "jpg".to_string(),
        "image/png" => "png".to_string(),
        "image/webp" => "webp".to_string(),
        _ => "bin".to_string(),
    }
}

fn extract_avatar_filename_stem(filename: &str) -> Option<String> {
    let raw_name = std::path::Path::new(filename)
        .file_stem()?
        .to_str()?
        .trim()
        .to_lowercase();

    let normalized = raw_name.replace('-', "");
    is_uuid_hex(&normalized).then_some(normalized)
}

fn is_uuid_hex(value: &str) -> bool {
    value.len() == 32 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub async fn get_image(
    State(state): State<AppState>,
    Path(filename): Path<String>,
) -> Result<Response, AppError> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(AppError::BadRequest("非法文件名".to_string()));
    }

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
