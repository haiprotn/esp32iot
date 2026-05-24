// src/network/wifi.rs — WiFi: kết nối, reconnect tự động, trạng thái

use std::time::Duration;

use anyhow::bail;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use log::{info, warn, error};

/// Số lần thử kết nối lại tối đa trước khi báo lỗi
const MAX_RETRY: u32 = 5;
/// Thời gian chờ giữa mỗi lần thử (ms)
const RETRY_DELAY_MS: u64 = 2000;
/// Timeout kết nối mỗi lần thử (ms)
const CONNECT_TIMEOUT_MS: u64 = 15_000;

pub struct WifiManager<'a> {
    wifi: BlockingWifi<EspWifi<'a>>,
    connected: bool,
    ssid: heapless::String<32>,
    ip: Option<std::net::Ipv4Addr>,
}

impl<'a> WifiManager<'a> {
    pub fn new(
        modem: esp_idf_hal::modem::Modem,
        sysloop: EspSystemEventLoop,
        nvs: EspDefaultNvsPartition,
    ) -> anyhow::Result<Self> {
        let esp_wifi = EspWifi::new(modem, sysloop.clone(), Some(nvs))?;
        let wifi = BlockingWifi::wrap(esp_wifi, sysloop)?;
        Ok(Self {
            wifi,
            connected: false,
            ssid: heapless::String::new(),
            ip: None,
        })
    }

    /// Kết nối WiFi — thử lại tối đa MAX_RETRY lần
    pub fn connect(&mut self, ssid: &str, password: &str) -> anyhow::Result<std::net::Ipv4Addr> {
        info!("[WiFi] Đang kết nối tới '{}'...", ssid);

        let ssid_str: heapless::String<32> = ssid.try_into()
            .map_err(|_| anyhow::anyhow!("SSID quá dài (max 32 ký tự)"))?;
        let pass_str: heapless::String<64> = password.try_into()
            .map_err(|_| anyhow::anyhow!("Password quá dài (max 64 ký tự)"))?;

        let config = Configuration::Client(ClientConfiguration {
            ssid: ssid_str.clone(),
            password: pass_str,
            auth_method: if password.is_empty() {
                AuthMethod::None
            } else {
                AuthMethod::WPA2Personal
            },
            ..Default::default()
        });

        self.wifi.set_configuration(&config)?;
        self.wifi.start()?;

        for attempt in 1..=MAX_RETRY {
            info!("[WiFi] Lần thử {}/{}", attempt, MAX_RETRY);

            match self.wifi.connect() {
                Ok(_) => {
                    info!("[WiFi] Connected! Đang lấy IP...");
                    self.wifi.wait_netif_up()?;

                    let ip_info = self.wifi.wifi().sta_netif().get_ip_info()?;
                    let ip = ip_info.ip;

                    info!("[WiFi] IP: {} | Gateway: {} | Mask: {}",
                        ip, ip_info.subnet.gateway, ip_info.subnet.mask);

                    self.connected = true;
                    self.ssid = ssid_str;
                    self.ip = Some(ip);
                    return Ok(ip);
                }
                Err(e) => {
                    warn!("[WiFi] Lần {} thất bại: {:?}", attempt, e);
                    if attempt < MAX_RETRY {
                        std::thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
                    }
                }
            }
        }

        bail!("[WiFi] Không thể kết nối sau {} lần thử", MAX_RETRY);
    }

    /// Kiểm tra kết nối và tự reconnect nếu mất
    /// Gọi định kỳ trong main loop (ví dụ mỗi 30 giây)
    pub fn ensure_connected(&mut self, ssid: &str, password: &str) -> anyhow::Result<()> {
        if self.wifi.is_connected()? {
            return Ok(());
        }

        warn!("[WiFi] Mất kết nối — đang reconnect...");
        self.connected = false;

        // Thử reconnect
        match self.connect(ssid, password) {
            Ok(ip) => {
                info!("[WiFi] Reconnected! IP: {}", ip);
                Ok(())
            }
            Err(e) => {
                error!("[WiFi] Reconnect thất bại: {:?}", e);
                Err(e)
            }
        }
    }

    /// Disconnect và dừng WiFi
    pub fn disconnect(&mut self) -> anyhow::Result<()> {
        self.wifi.disconnect()?;
        self.wifi.stop()?;
        self.connected = false;
        self.ip = None;
        info!("[WiFi] Disconnected");
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn ip_address(&self) -> Option<std::net::Ipv4Addr> {
        self.ip
    }

    pub fn ssid(&self) -> &str {
        self.ssid.as_str()
    }

    /// Lấy modem để dùng SoftAP provisioning (cần stop WiFi trước)
    pub fn into_modem(self) -> anyhow::Result<esp_idf_hal::modem::Modem> {
        // Không thể lấy modem ra dễ dàng trong esp-idf-svc
        // Giải pháp: dùng Arc<Mutex<EspWifi>> và reset config
        Err(anyhow::anyhow!("Chưa implement — dùng restart provisioning flow"))
    }
}
