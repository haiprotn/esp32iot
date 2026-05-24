// src/dp/codec.rs — Encode/decode DP sang/từ JSON (cho HTTP API)

use serde_json::{json, Value};
use super::types::DpValue;

/// Encode một DpValue thành JSON Value
pub fn encode_dp_value(value: &DpValue) -> Value {
    match value {
        DpValue::Bool(v) => json!(v),
        DpValue::Int(v)  => json!(v),
        DpValue::Enum(v) => json!(v),
        DpValue::Str(v)  => json!(v),
    }
}

/// Decode JSON Value thành DpValue
/// tag: "bool" | "int" | "enum" | "string" — để biết kiểu cần parse
pub fn decode_dp_value(raw: &Value, tag: &str) -> anyhow::Result<DpValue> {
    match tag {
        "bool" => {
            raw.as_bool()
                .map(DpValue::Bool)
                .ok_or_else(|| anyhow::anyhow!("Cần giá trị boolean, nhận: {}", raw))
        }
        "int" => {
            raw.as_i64()
                .map(|v| DpValue::Int(v as i32))
                .ok_or_else(|| anyhow::anyhow!("Cần giá trị integer, nhận: {}", raw))
        }
        "enum" => {
            raw.as_u64()
                .map(|v| DpValue::Enum(v as u8))
                .ok_or_else(|| anyhow::anyhow!("Cần giá trị u8 cho enum, nhận: {}", raw))
        }
        "string" => {
            raw.as_str()
                .map(|v| DpValue::Str(v.to_string()))
                .ok_or_else(|| anyhow::anyhow!("Cần giá trị string, nhận: {}", raw))
        }
        _ => Err(anyhow::anyhow!("Tag không hợp lệ: {}", tag)),
    }
}

/// Encode toàn bộ snapshot thành JSON object
/// Format: { "dp_id": value, ... }
pub fn encode_snapshot(snapshot: &[(u8, DpValue)]) -> Value {
    let mut obj = serde_json::Map::new();
    for (id, value) in snapshot {
        obj.insert(id.to_string(), encode_dp_value(value));
    }
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_bool() {
        assert_eq!(encode_dp_value(&DpValue::Bool(true)), json!(true));
        assert_eq!(encode_dp_value(&DpValue::Bool(false)), json!(false));
    }

    #[test]
    fn test_encode_int() {
        assert_eq!(encode_dp_value(&DpValue::Int(42)), json!(42));
    }

    #[test]
    fn test_decode_bool() {
        let v = decode_dp_value(&json!(true), "bool").unwrap();
        assert_eq!(v, DpValue::Bool(true));
    }

    #[test]
    fn test_decode_int_bounds() {
        let v = decode_dp_value(&json!(300), "int").unwrap();
        assert_eq!(v, DpValue::Int(300));
    }
}
