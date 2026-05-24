// src/provisioning/softap_prov.rs — SoftAP + Captive Portal provisioning
//
// Khi không có WiFi config:
//   1. Bật SoftAP với SSID: "SmartHome-XXXXXX"
//   2. Client kết nối vào AP
//   3. Mở browser → redirect tới 192.168.4.1 (captive portal)
//   4. User nhập SSID + password → POST /api/provision
//   5. Device lưu vào NVS → restart → kết nối WiFi

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::http::server::{Configuration as HttpConfig, EspHttpServer};
use esp_idf_svc::io::Write;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{AccessPointConfiguration, Configuration, EspWifi};
use log::info;
use std::sync::{Arc, Mutex};

/// Thời gian timeout SoftAP (ms) — tự tắt sau 5 phút nếu không ai cấu hình
const SOFTAP_TIMEOUT_MS: u64 = 5 * 60 * 1000;

const PROVISION_HTML: &str = r#"<!DOCTYPE html>
<html lang="vi">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Cấu hình WiFi</title>
  <style>
    * { box-sizing: border-box; }
    body { font-family: -apple-system, sans-serif; background: #1a1a2e; color: #eee;
           display: flex; align-items: center; justify-content: center; min-height: 100vh; }
    .card { background: #16213e; border-radius: 16px; padding: 32px; width: 320px; }
    h2 { color: #e94560; margin-bottom: 24px; text-align: center; }
    label { display: block; margin-bottom: 4px; font-size: 0.85rem; color: #aaa; }
    input { width: 100%; padding: 10px 14px; background: #0f3460; border: 1px solid #e94560;
            border-radius: 8px; color: #eee; font-size: 1rem; margin-bottom: 16px; }
    button { width: 100%; padding: 12px; background: #e94560; border: none;
             border-radius: 8px; color: white; font-size: 1rem; cursor: pointer; }
    .msg { text-align: center; margin-top: 12px; font-size: 0.85rem; }
  </style>
</head>
<body>
  <div class="card">
    <h2>Cấu hình WiFi</h2>
    <form id="form">
      <label>Tên mạng WiFi (SSID)</label>
      <input type="text" id="ssid" placeholder="Nhập SSID..." required maxlength="32">
      <label>Mật khẩu</label>
      <input type="password" id="pass" placeholder="Nhập password..." maxlength="64">
      <button type="submit">Kết nối</button>
    </form>
    <div class="msg" id="msg"></div>
  </div>
  <script>
    document.getElementById('form').onsubmit = async (e) => {
      e.preventDefault();
      const ssid = document.getElementById('ssid').value;
      const pass = document.getElementById('pass').value;
      document.getElementById('msg').textContent = 'Đang kết nối...';
      try {
        const r = await fetch('/api/provision', {
          method: 'POST',
          headers: {'Content-Type': 'application/json'},
          body: JSON.stringify({ssid, password: pass})
        });
        const d = await r.json();
        if (d.ok) {
          document.getElementById('msg').textContent = 'Thành công! Thiết bị đang khởi động lại...';
        } else {
          document.getElementById('msg').textContent = 'Lỗi: ' + (d.error || 'Không xác định');
        }
      } catch(e) {
        document.getElementById('msg').textContent = 'Lỗi kết nối';
      }
    };
  </script>
</body>
</html>"#;

/// Credentials được submit qua form
#[derive(serde::Deserialize)]
struct ProvisionRequest {
    ssid: String,
    password: String,
}

pub struct SoftApProvisioning {
    _server: EspHttpServer<'static>,
    credentials: Arc<Mutex<Option<(String, String)>>>,
    started_at: std::time::Instant,
}

impl SoftApProvisioning {
    /// Khởi động SoftAP + HTTP server để provisioning
    pub fn start(
        modem: esp_idf_hal::modem::Modem,
        sysloop: EspSystemEventLoop,
        nvs: EspDefaultNvsPartition,
        ap_ssid: &str,
    ) -> anyhow::Result<Self> {
        // Bật SoftAP
        let mut wifi = EspWifi::new(modem, sysloop, Some(nvs))?;

        let ap_ssid_hs: heapless::String<32> = ap_ssid.try_into()
            .map_err(|_| anyhow::anyhow!("AP SSID quá dài"))?;

        wifi.set_configuration(&Configuration::AccessPoint(AccessPointConfiguration {
            ssid: ap_ssid_hs,
            password: heapless::String::new(), // Mở, không mật khẩu
            channel: 6,
            max_connections: 4,
            ..Default::default()
        }))?;
        wifi.start()?;

        info!("[SoftAP] Started: SSID='{}' | IP: 192.168.4.1", ap_ssid);

        let credentials: Arc<Mutex<Option<(String, String)>>> = Arc::new(Mutex::new(None));
        let cred_clone = credentials.clone();

        // HTTP server trên port 80
        let mut server = EspHttpServer::new(&HttpConfig { stack_size: 6144, ..Default::default() })?;

        // Captive portal redirect
        server.fn_handler("/", esp_idf_svc::http::Method::Get, |req| {
            req.into_ok_response()?.write_all(PROVISION_HTML.as_bytes())?;
            Ok::<(), anyhow::Error>(())
        })?;

        // POST /api/provision — nhận credentials
        server.fn_handler("/api/provision", esp_idf_svc::http::Method::Post, move |mut req| {
            let mut body = Vec::new();
            let mut buf = [0u8; 256];
            loop {
                match req.read(&mut buf)? {
                    0 => break,
                    n => body.extend_from_slice(&buf[..n]),
                }
            }

            let result = serde_json::from_slice::<ProvisionRequest>(&body);
            let mut resp = req.into_ok_response()?;

            match result {
                Ok(prov) if !prov.ssid.is_empty() => {
                    info!("[Provision] Received SSID: '{}'", prov.ssid);
                    *cred_clone.lock().unwrap() = Some((prov.ssid, prov.password));
                    resp.write_all(b"{\"ok\":true}")?;
                }
                Ok(_) => {
                    resp.write_all(b"{\"ok\":false,\"error\":\"SSID empty\"}")?;
                }
                Err(e) => {
                    resp.write_all(format!("{{\"ok\":false,\"error\":\"{}\"}}", e).as_bytes())?;
                }
            }
            Ok::<(), anyhow::Error>(())
        })?;

        Ok(Self {
            _server: server,
            credentials,
            started_at: std::time::Instant::now(),
        })
    }

    /// Kiểm tra đã nhận được credentials chưa
    pub fn get_credentials(&self) -> Option<(String, String)> {
        self.credentials.lock().unwrap().clone()
    }

    /// Kiểm tra đã timeout chưa
    pub fn is_timed_out(&self) -> bool {
        self.started_at.elapsed().as_millis() as u64 >= SOFTAP_TIMEOUT_MS
    }
}
