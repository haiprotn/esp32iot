// src/network/mdns.rs — mDNS: device có thể tìm bằng {hostname}.local

use esp_idf_svc::mdns::EspMdns;
use log::info;

pub struct MdnsService {
    _mdns: EspMdns,
}

impl MdnsService {
    /// Khởi tạo mDNS
    /// hostname: tên thiết bị (ví dụ "switch-1g-abc123")
    /// Thiết bị sẽ trả lời cho {hostname}.local
    pub fn new(hostname: &str, product_name: &str) -> anyhow::Result<Self> {
        let mut mdns = EspMdns::take()?;

        mdns.set_hostname(hostname)?;
        mdns.set_instance_name(product_name)?;

        // Đăng ký HTTP service
        mdns.add_service(
            None,
            "_http",
            "_tcp",
            80,
            &[("path", "/"), ("product", product_name)],
        )?;

        info!("[mDNS] Hostname: {}.local", hostname);
        info!("[mDNS] Service: _http._tcp port 80");

        Ok(Self { _mdns: mdns })
    }
}

/// Tạo hostname duy nhất từ MAC address
/// Ví dụ: "switch-1g-a1b2c3"
pub fn make_hostname(product_id: &str) -> String {
    // Lấy 3 byte cuối MAC để tạo unique suffix
    let mac = get_mac_suffix();
    format!("{}-{}", product_id.replace('_', "-"), mac)
}

fn get_mac_suffix() -> String {
    use esp_idf_sys::{esp_read_mac, esp_mac_type_t_ESP_MAC_WIFI_STA};
    let mut mac = [0u8; 6];
    unsafe {
        esp_read_mac(mac.as_mut_ptr(), esp_mac_type_t_ESP_MAC_WIFI_STA);
    }
    // Chỉ lấy 3 byte cuối
    format!("{:02x}{:02x}{:02x}", mac[3], mac[4], mac[5])
}
