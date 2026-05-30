use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, Default)]
pub struct HistoryRecord {
    #[serde(skip)]
    pub id: i64,
    #[serde(rename = "id")]
    pub public_id: String,
    pub r#type: String,
    #[serde(skip)]
    pub item_id: i64,
    #[serde(rename = "item_id")]
    pub item_public_id: String,
    pub item_name: String,
    pub qty: i32,
    pub from_location: Option<String>,
    pub to_location: Option<String>,
    pub reason: Option<String>,
    pub remark: Option<String>,
    pub time: String,
    pub version: i64,
    #[serde(rename = "is_deleted")]
    pub is_deleted: i16,
}

#[derive(Debug, Deserialize)]
pub struct HistoryQueryParams {
    pub r#type: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i32,
    pub before: Option<String>,
    pub after: Option<String>,
}

fn default_limit() -> i32 { 20 }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_record_serialization() {
        let record = HistoryRecord {
            id: 123, public_id: "h1".into(), r#type: "in".into(),
            item_id: 456, item_public_id: "i1".into(), item_name: "test".into(),
            qty: 5, from_location: Some("仓库A".into()), to_location: Some("仓库B".into()),
            reason: Some("补货".into()), remark: None,
            time: "2024-01-01".into(), version: 1, is_deleted: 0,
        };
        let json = serde_json::to_string(&record).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["id"], "h1");
        assert_eq!(parsed["item_id"], "i1");
        assert_eq!(parsed["type"], "in");
        assert_eq!(parsed["item_name"], "test");
        assert_eq!(parsed["from_location"], "仓库A");
        assert_eq!(parsed["to_location"], "仓库B");
        assert_eq!(parsed["qty"], 5);
        // is_deleted 应被重命名为 is_deleted
        assert_eq!(parsed["is_deleted"], 0);
    }

    #[test]
    fn test_history_query_params_defaults() {
        let json = r#"{}"#;
        let params: HistoryQueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.r#type, None);
        assert_eq!(params.limit, 20);
        assert_eq!(params.before, None);
        assert_eq!(params.after, None);
    }

    #[test]
    fn test_history_query_params_full() {
        let json = r#"{"type": "in", "limit": 50, "before": "2024-06-01", "after": "2024-01-01"}"#;
        let params: HistoryQueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.r#type, Some("in".into()));
        assert_eq!(params.limit, 50);
        assert_eq!(params.before, Some("2024-06-01".into()));
        assert_eq!(params.after, Some("2024-01-01".into()));
    }

    #[test]
    fn test_history_record_with_null_fields() {
        let record = HistoryRecord {
            id: 0, public_id: "h2".into(), r#type: "out".into(),
            item_id: 0, item_public_id: "i2".into(), item_name: "test".into(),
            qty: 1, from_location: None, to_location: None, reason: None,
            remark: None, time: "now".into(), version: 0, is_deleted: 0,
        };
        let json = serde_json::to_string(&record).unwrap();
        // Option<String> 为 None 时应在 JSON 中为 null
        assert!(json.contains("null"));
    }

    #[test]
    fn test_history_record_type_field() {
        let record = HistoryRecord {
            id: 0, public_id: "h3".into(), r#type: "in".into(),
            item_id: 0, item_public_id: "i3".into(), item_name: "test".into(),
            qty: 1, from_location: None, to_location: None, reason: None,
            remark: None, time: "now".into(), version: 0, is_deleted: 0,
        };
        let serialized = serde_json::to_string(&record).unwrap();
        assert!(serialized.contains(r#""type":"#));
    }

    #[test]
    fn test_history_record_skips_internal_fields() {
        let record = HistoryRecord {
            id: 999, public_id: "h1".into(), r#type: "in".into(),
            item_id: 888, item_public_id: "i1".into(), item_name: "test".into(),
            qty: 1, from_location: None, to_location: None, reason: None,
            remark: None, time: "now".into(), version: 0, is_deleted: 0,
        };
        let json = serde_json::to_string(&record).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("id").is_none() || parsed["id"] == "h1");
        assert!(parsed.get("item_id").is_none() || parsed["item_id"] == "i1");
    }

    #[test]
    fn test_history_query_params_invalid_limit_type() {
        let json = r#"{"limit": "not_a_number"}"#;
        let result = serde_json::from_str::<HistoryQueryParams>(json);
        assert!(result.is_err());
    }
}
