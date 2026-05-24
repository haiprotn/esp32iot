// src/network/mod.rs

pub mod http_server;
#[cfg(any(esp_idf_comp_mdns_enabled, esp_idf_comp_espressif__mdns_enabled))]
pub mod mdns;
pub mod wifi;

pub use http_server::HttpServer;
#[cfg(any(esp_idf_comp_mdns_enabled, esp_idf_comp_espressif__mdns_enabled))]
pub use mdns::MdnsService;
pub use wifi::WifiManager;

pub fn get_mac_suffix() -> String {
    use esp_idf_sys::{esp_read_mac, esp_mac_type_t_ESP_MAC_WIFI_STA};
    let mut mac = [0u8; 6];
    unsafe { esp_read_mac(mac.as_mut_ptr(), esp_mac_type_t_ESP_MAC_WIFI_STA) };
    format!("{:02x}{:02x}{:02x}", mac[3], mac[4], mac[5])
}

pub fn make_hostname(product_id: &str) -> String {
    format!("{}-{}", product_id.replace('_', "-"), get_mac_suffix())
}
