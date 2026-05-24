// src/storage/mod.rs — NVS wrapper: lưu WiFi credentials và trạng thái DP

use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};
use log::{info, warn};

/// Namespace trong NVS
const NVS_NAMESPACE: &str = "smarthome";
/// Key cho SSID
const KEY_WIFI_SSID: &str = "wifi_ssid";
/// Key cho password
const KEY_WIFI_PASS: &str = "wifi_pass";
/// Prefix cho DP state: "dp_" + id
const KEY_DP_PREFIX: &str = "dp_";

pub struct NvsStorage {
    nvs: EspNvs<NvsDefault>,
}

impl NvsStorage {
    pub fn new(partition: EspDefaultNvsPartition) -> anyhow::Result<Self> {
        let nvs = EspNvs::new(partition, NVS_NAMESPACE, true)?;
        info!("[NVS] Storage initialized (namespace: '{}')", NVS_NAMESPACE);
        Ok(Self { nvs })
    }

    // ─── WiFi ─────────────────────────────────────────────────────────────────

    /// Lưu WiFi credentials
    pub fn save_wifi(&mut self, ssid: &str, password: &str) -> anyhow::Result<()> {
        self.nvs.set_str(KEY_WIFI_SSID, ssid)?;
        self.nvs.set_str(KEY_WIFI_PASS, password)?;
        info!("[NVS] WiFi credentials saved for SSID: '{}'", ssid);
        Ok(())
    }

    /// Đọc SSID đã lưu
    pub fn get_wifi_ssid(&self) -> anyhow::Result<Option<String>> {
        let mut buf = [0u8; 33];
        match self.nvs.get_str(KEY_WIFI_SSID, &mut buf)? {
            Some(s) if !s.is_empty() => Ok(Some(s.to_string())),
            _ => Ok(None),
        }
    }

    /// Đọc password đã lưu
    pub fn get_wifi_password(&self) -> anyhow::Result<Option<String>> {
        let mut buf = [0u8; 65];
        match self.nvs.get_str(KEY_WIFI_PASS, &mut buf)? {
            Some(s) => Ok(Some(s.to_string())),
            None => Ok(None),
        }
    }

    /// Xóa WiFi credentials (khi long press reset)
    pub fn clear_wifi(&mut self) -> anyhow::Result<()> {
        let _ = self.nvs.remove(KEY_WIFI_SSID);
        let _ = self.nvs.remove(KEY_WIFI_PASS);
        warn!("[NVS] WiFi credentials cleared");
        Ok(())
    }

    /// Kiểm tra có WiFi đã cấu hình chưa
    pub fn has_wifi_config(&self) -> bool {
        self.get_wifi_ssid().ok().flatten().is_some()
    }

    // ─── DP State ─────────────────────────────────────────────────────────────

    /// Lưu trạng thái boolean của một DP (cho relay state)
    pub fn save_dp_bool(&mut self, dp_id: u8, value: bool) -> anyhow::Result<()> {
        let key = format!("{}{}", KEY_DP_PREFIX, dp_id);
        self.nvs.set_u8(&key, if value { 1 } else { 0 })?;
        Ok(())
    }

    /// Đọc trạng thái boolean của một DP
    pub fn get_dp_bool(&self, dp_id: u8) -> anyhow::Result<Option<bool>> {
        let key = format!("{}{}", KEY_DP_PREFIX, dp_id);
        match self.nvs.get_u8(&key)? {
            Some(v) => Ok(Some(v != 0)),
            None => Ok(None),
        }
    }

    /// Lưu trạng thái i32 của một DP
    pub fn save_dp_int(&mut self, dp_id: u8, value: i32) -> anyhow::Result<()> {
        let key = format!("{}{}", KEY_DP_PREFIX, dp_id);
        self.nvs.set_i32(&key, value)?;
        Ok(())
    }

    /// Đọc trạng thái i32 của một DP
    pub fn get_dp_int(&self, dp_id: u8) -> anyhow::Result<Option<i32>> {
        let key = format!("{}{}", KEY_DP_PREFIX, dp_id);
        Ok(self.nvs.get_i32(&key)?)
    }
}
