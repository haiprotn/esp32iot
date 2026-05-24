// src/dp/types.rs — Định nghĩa kiểu dữ liệu Data Point (Tuya-compatible)

/// Kiểu dữ liệu của một Data Point
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DpType {
    /// Bật/tắt (boolean)
    Bool,
    /// Số nguyên có giới hạn min/max
    Int { min: i32, max: i32 },
    /// Chuỗi định nghĩa sẵn (enum) — lưu dưới dạng index u8
    Enum(Vec<String>),
    /// Chuỗi văn bản tự do
    StringType { max_len: usize },
}

/// Định nghĩa một Data Point — const, nằm trên ROM
#[derive(Debug, Clone)]
pub struct DpDefinition {
    /// ID duy nhất (1–255, theo Tuya)
    pub id: u8,
    /// Tên human-readable
    pub name: &'static str,
    /// Kiểu dữ liệu
    pub dp_type: DpType,
    /// Client có thể ghi (true) hay chỉ đọc (false)
    pub writable: bool,
}

/// Giá trị thực tế của một DP tại thời điểm hiện tại
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DpValue {
    Bool(bool),
    Int(i32),
    Enum(u8),
    Str(String),
}

impl DpValue {
    /// Trả về giá trị bool, None nếu không phải Bool
    pub fn as_bool(&self) -> Option<bool> {
        if let DpValue::Bool(v) = self { Some(*v) } else { None }
    }

    /// Trả về giá trị int, None nếu không phải Int
    pub fn as_int(&self) -> Option<i32> {
        if let DpValue::Int(v) = self { Some(*v) } else { None }
    }
}

impl std::fmt::Display for DpValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DpValue::Bool(v) => write!(f, "{}", v),
            DpValue::Int(v)  => write!(f, "{}", v),
            DpValue::Enum(v) => write!(f, "{}", v),
            DpValue::Str(v)  => write!(f, "{}", v),
        }
    }
}
