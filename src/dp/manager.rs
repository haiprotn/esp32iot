// src/dp/manager.rs — Quản lý trạng thái và callback cho các Data Point

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use log::{info, warn};

use super::types::{DpDefinition, DpValue};

/// Callback khi một DP thay đổi giá trị
/// dp_id: ID của DP thay đổi
/// old_value: Giá trị cũ (None nếu lần đầu set)
/// new_value: Giá trị mới
pub type DpChangeCallback = Box<dyn Fn(u8, Option<&DpValue>, &DpValue) + Send + Sync>;

/// Quản lý toàn bộ trạng thái DP của thiết bị
pub struct DpManager {
    definitions: &'static [DpDefinition],
    state: Arc<Mutex<HashMap<u8, DpValue>>>,
    callbacks: Vec<DpChangeCallback>,
}

impl DpManager {
    pub fn new(definitions: &'static [DpDefinition]) -> Self {
        let mut initial_state = HashMap::new();
        for dp in definitions {
            // Khởi tạo giá trị mặc định an toàn
            let default_val = match &dp.dp_type {
                super::types::DpType::Bool            => DpValue::Bool(false),
                super::types::DpType::Int { min, .. } => DpValue::Int(*min),
                super::types::DpType::Enum(_)         => DpValue::Enum(0),
                super::types::DpType::StringType { .. } => DpValue::Str(String::new()),
            };
            initial_state.insert(dp.id, default_val);
        }

        info!("DpManager: {} data points initialized", definitions.len());
        Self {
            definitions,
            state: Arc::new(Mutex::new(initial_state)),
            callbacks: Vec::new(),
        }
    }

    /// Đăng ký callback khi bất kỳ DP nào thay đổi
    pub fn on_change(&mut self, cb: DpChangeCallback) {
        self.callbacks.push(cb);
    }

    /// Set giá trị DP — validate trước khi set
    /// Trả về Err nếu DP không tồn tại, không writable, hoặc giá trị không hợp lệ
    pub fn set(&mut self, dp_id: u8, value: DpValue) -> anyhow::Result<()> {
        // Tìm definition
        let def = self.definitions.iter()
            .find(|d| d.id == dp_id)
            .ok_or_else(|| anyhow::anyhow!("DP {} không tồn tại", dp_id))?;

        // Validate kiểu dữ liệu
        self.validate_value(def, &value)?;

        let old_value = {
            let mut state = self.state.lock().unwrap();
            let old = state.get(&dp_id).cloned();
            state.insert(dp_id, value.clone());
            old
        };

        info!("DP {} '{}': {:?} → {:?}", dp_id, def.name, old_value, value);

        // Gọi callbacks
        for cb in &self.callbacks {
            cb(dp_id, old_value.as_ref(), &value);
        }

        Ok(())
    }

    /// Đọc giá trị hiện tại của một DP
    pub fn get(&self, dp_id: u8) -> Option<DpValue> {
        self.state.lock().unwrap().get(&dp_id).cloned()
    }

    /// Trả về snapshot toàn bộ state dưới dạng Vec<(id, value)>
    pub fn snapshot(&self) -> Vec<(u8, DpValue)> {
        let state = self.state.lock().unwrap();
        let mut result: Vec<_> = state.iter().map(|(&k, v)| (k, v.clone())).collect();
        result.sort_by_key(|(id, _)| *id);
        result
    }

    /// Validate giá trị theo định nghĩa DP
    fn validate_value(&self, def: &DpDefinition, value: &DpValue) -> anyhow::Result<()> {
        match (&def.dp_type, value) {
            (super::types::DpType::Bool, DpValue::Bool(_)) => Ok(()),
            (super::types::DpType::Int { min, max }, DpValue::Int(v)) => {
                if *v >= *min && *v <= *max {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("DP {}: giá trị {} ngoài range [{}, {}]", def.id, v, min, max))
                }
            }
            (super::types::DpType::Enum(variants), DpValue::Enum(idx)) => {
                if (*idx as usize) < variants.len() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("DP {}: enum index {} vượt quá số variants {}", def.id, idx, variants.len()))
                }
            }
            (super::types::DpType::StringType { max_len }, DpValue::Str(s)) => {
                if s.len() <= *max_len {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("DP {}: chuỗi dài {} > max {}", def.id, s.len(), max_len))
                }
            }
            _ => {
                warn!("DP {}: kiểu dữ liệu không khớp", def.id);
                Err(anyhow::anyhow!("DP {}: kiểu dữ liệu không khớp với định nghĩa", def.id))
            }
        }
    }

    /// Lấy danh sách definitions (để serialize schema)
    pub fn definitions(&self) -> &'static [DpDefinition] {
        self.definitions
    }
}
