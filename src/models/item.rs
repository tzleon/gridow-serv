use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Default)]
pub struct Item {
    #[serde(skip)]
    pub id: i64,
    #[serde(rename = "id")]
    pub public_id: String,
    pub name: String,
    pub icon: String,
    pub qty: i32,
    pub location: String,
    #[serde(skip)]
    pub location_id: Option<i64>,
    pub category: String,
    pub tags: String,
    pub barcode: String,
    pub photos: String,
    pub photo_uri: String,
    pub buy_date: String,
    pub expiry: String,
    pub remark: String,
    pub track_low_stock: bool,
    #[serde(skip)]
    pub owner_id: i64,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
    #[serde(rename = "is_deleted")]
    pub is_deleted: i16,
}

#[derive(Debug, Deserialize)]
pub struct ItemCreateRequest {
    pub name: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default = "default_qty")]
    pub qty: i32,
    pub location_id: Option<String>,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub barcode: String,
    #[serde(default)]
    pub photo_uri: String,
    #[serde(default)]
    pub buy_date: String,
    #[serde(default = "default_expiry")]
    pub expiry: String,
    #[serde(default)]
    pub remark: String,
    #[serde(default)]
    pub track_low_stock: bool,
}

#[derive(Debug, Deserialize)]
pub struct ItemUpdateRequest {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub qty: Option<i32>,
    pub location_id: Option<Option<String>>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub barcode: Option<String>,
    pub photo_uri: Option<String>,
    pub buy_date: Option<String>,
    pub expiry: Option<String>,
    pub remark: Option<String>,
    pub track_low_stock: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct OutboundRequest {
    pub qty: i32,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct TransferRequest {
    pub target_space_id: String,
    #[serde(default = "default_one")]
    pub qty: i32,
}

#[derive(Debug, Deserialize)]
pub struct ItemQueryParams {
    pub category: Option<String>,
    pub keyword: Option<String>,
    pub space_id: Option<String>,
    #[serde(default = "default_page")]
    pub page: i32,
    #[serde(default = "default_page_size")]
    pub page_size: i32,
    #[serde(default = "default_sort_by")]
    pub sort_by: String,
    #[serde(default = "default_sort_order")]
    pub sort_order: String,
}

#[derive(Debug, Serialize)]
pub struct ItemListResponse {
    pub items: Vec<Item>,
    pub total: i32,
    pub page: i32,
    pub page_size: i32,
}

fn default_icon() -> String { "📦".to_string() }
fn default_qty() -> i32 { 1 }
fn default_category() -> String { "daily".to_string() }
fn default_expiry() -> String { "-".to_string() }
fn default_page() -> i32 { 1 }
fn default_page_size() -> i32 { 20 }
fn default_sort_by() -> String { "updatedAt".to_string() }
fn default_sort_order() -> String { "desc".to_string() }
fn default_one() -> i32 { 1 }

#[cfg(test)]
pub(crate) fn now_string() -> String {
    chrono::Utc::now().naive_utc().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── now_string ───────────────────────────────────────

    #[test]
    fn test_now_string_format() {
        let s = now_string();
        // 格式应为 YYYY-MM-DD HH:MM:SS
        assert_eq!(s.len(), 19, "Expected format YYYY-MM-DD HH:MM:SS, got {}", s);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
        assert_eq!(&s[10..11], " ");
        assert_eq!(&s[13..14], ":");
        assert_eq!(&s[16..17], ":");
    }

    // ── Item 序列化 ──────────────────────────────────────

    #[test]
    fn test_item_serialization_skips_internal_fields() {
        let item = Item {
            id: 12345, public_id: "pub_item1".into(), name: "test".into(), icon: "📦".into(),
            qty: 5, location: "room".into(), location_id: Some(1), category: "daily".into(),
            tags: "[]".into(), barcode: "123".into(), photos: "[]".into(), photo_uri: "".into(),
            buy_date: "".into(), expiry: "-".into(), remark: "".into(), track_low_stock: false,
            owner_id: 999, created_at: "now".into(), updated_at: "now".into(), version: 3, is_deleted: 0,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["id"], "pub_item1");
        // owner_id 应被跳过
        assert!(parsed.get("owner_id").is_none(), "owner_id should be skipped");
    }

    #[test]
    fn test_item_deserialization_from_row() {
        // 测试 Item 能正确从 sqlx row 反序列化
        // 这里仅测试 serde 反序列化能力（非 sqlx）
        let json = r#"{
            "name": "矿泉水",
            "icon": "💧",
            "qty": 24,
            "category": "food"
        }"#;
        let req: ItemCreateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "矿泉水");
        assert_eq!(req.icon, "💧");
        assert_eq!(req.qty, 24);
        assert_eq!(req.category, "food");
        // 默认值
        assert_eq!(req.tags, Vec::<String>::new());
        assert_eq!(req.barcode, "");
        assert_eq!(req.track_low_stock, false);
    }

    #[test]
    fn test_item_create_request_defaults() {
        let json = r#"{"name": "默认值测试"}"#;
        let req: ItemCreateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "默认值测试");
        assert_eq!(req.icon, "📦");
        assert_eq!(req.qty, 1);
        assert_eq!(req.category, "daily");
        assert_eq!(req.expiry, "-");
    }

    #[test]
    fn test_item_update_request_partial() {
        let json = r#"{"name": "新名称", "qty": 10}"#;
        let req: ItemUpdateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, Some("新名称".into()));
        assert_eq!(req.qty, Some(10));
        assert_eq!(req.icon, None);
        assert_eq!(req.location_id, None);
    }

    #[test]
    fn test_item_update_request_location_unset() {
        let json = r#"{"location_id": null}"#;
        let req: ItemUpdateRequest = serde_json::from_str(json).unwrap();
        // `null` 被 serde 解析为 None（而不是 Some(None)）
        assert_eq!(req.location_id, None);
    }

    // ── OutboundRequest ──────────────────────────────────

    #[test]
    fn test_outbound_request() {
        let json = r#"{"qty": 3, "reason": "出库测试"}"#;
        let req: OutboundRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.qty, 3);
        assert_eq!(req.reason, "出库测试");
    }

    #[test]
    fn test_outbound_request_default_reason() {
        let json = r#"{"qty": 1}"#;
        let req: OutboundRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.qty, 1);
        assert_eq!(req.reason, "");
    }

    // ── TransferRequest ──────────────────────────────────

    #[test]
    fn test_transfer_request() {
        let json = r#"{"target_space_id": "space123", "qty": 2}"#;
        let req: TransferRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.target_space_id, "space123");
        assert_eq!(req.qty, 2);
    }

    #[test]
    fn test_transfer_request_default_qty() {
        let json = r#"{"target_space_id": "space123"}"#;
        let req: TransferRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.target_space_id, "space123");
        assert_eq!(req.qty, 1);
    }

    // ── ItemQueryParams ──────────────────────────────────

    #[test]
    fn test_item_query_params_defaults() {
        let json = r#"{}"#;
        let params: ItemQueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.page, 1);
        assert_eq!(params.page_size, 20);
        assert_eq!(params.sort_by, "updatedAt");
        assert_eq!(params.sort_order, "desc");
        assert_eq!(params.category, None);
        assert_eq!(params.keyword, None);
        assert_eq!(params.space_id, None);
    }

    #[test]
    fn test_item_query_params_full() {
        let json = r#"{
            "category": "food",
            "keyword": "水",
            "space_id": "space1",
            "page": 2,
            "page_size": 10,
            "sort_by": "name",
            "sort_order": "asc"
        }"#;
        let params: ItemQueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.category, Some("food".into()));
        assert_eq!(params.keyword, Some("水".into()));
        assert_eq!(params.space_id, Some("space1".into()));
        assert_eq!(params.page, 2);
        assert_eq!(params.page_size, 10);
        assert_eq!(params.sort_by, "name");
        assert_eq!(params.sort_order, "asc");
    }

    // ── ItemListResponse ─────────────────────────────────

    #[test]
    fn test_item_list_response_serialization() {
        let resp = ItemListResponse {
            items: vec![],
            total: 0,
            page: 1,
            page_size: 20,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""total":0"#));
        assert!(json.contains(r#""page":1"#));
    }

    #[test]
    fn test_item_create_request_missing_name() {
        let json = r#"{"icon": "📦"}"#;
        let result = serde_json::from_str::<ItemCreateRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_item_create_request_invalid_json() {
        let json = r#"not json"#;
        let result = serde_json::from_str::<ItemCreateRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_item_create_request_wrong_type_for_qty() {
        let json = r#"{"name": "test", "qty": "not_a_number"}"#;
        let result = serde_json::from_str::<ItemCreateRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_outbound_request_missing_qty() {
        let json = r#"{"reason": "测试"}"#;
        let result = serde_json::from_str::<OutboundRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_transfer_request_missing_target_space_id() {
        let json = r#"{"qty": 1}"#;
        let result = serde_json::from_str::<TransferRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_item_update_request_location_id_set_new() {
        let json = r#"{"location_id": "space_abc"}"#;
        let result = serde_json::from_str::<ItemUpdateRequest>(json).unwrap();
        assert_eq!(result.location_id, Some(Some("space_abc".into())));
    }

    #[test]
    fn test_item_create_request_empty_name() {
        let json = r#"{"name": ""}"#;
        let req: ItemCreateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "");
    }

    #[test]
    fn test_item_create_request_with_tags() {
        let json = r#"{"name": "test", "tags": ["标签1", "标签2", "标签3"]}"#;
        let req: ItemCreateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.tags, vec!["标签1", "标签2", "标签3"]);
    }
}
