// src/network/http_server.rs — HTTP REST API server nội bộ (Giai đoạn 1)
//
// Endpoints:
//   GET  /              → Web UI (HTML)
//   GET  /api/info      → Thông tin thiết bị (product_id, version, ip)
//   GET  /api/dp        → Toàn bộ DP state (JSON)
//   POST /api/dp/{id}   → Set giá trị DP (body: {"value": ...})
//   POST /api/restart   → Restart thiết bị

use std::sync::{Arc, Mutex};

use esp_idf_svc::http::server::{Configuration as HttpConfig, EspHttpServer};
use esp_idf_svc::io::Write;
use log::info;

use crate::dp::{manager::DpManager, codec};

/// Web UI nhúng — HTML/CSS/JS tối giản
/// Hiển thị danh sách relay và nút toggle
const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="vi">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Smart Home</title>
  <style>
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body { font-family: -apple-system, sans-serif; background: #1a1a2e; color: #eee; min-height: 100vh; }
    .header { background: #16213e; padding: 20px; text-align: center; }
    .header h1 { color: #0f3460; font-size: 1.4rem; }
    .header p  { color: #e94560; font-size: 0.85rem; margin-top: 4px; }
    .container { max-width: 480px; margin: 20px auto; padding: 0 16px; }
    .card { background: #16213e; border-radius: 12px; padding: 20px; margin-bottom: 16px; }
    .dp-row { display: flex; align-items: center; justify-content: space-between; padding: 12px 0; border-bottom: 1px solid #0f3460; }
    .dp-row:last-child { border-bottom: none; }
    .dp-name { font-size: 1rem; }
    .dp-val  { font-size: 0.85rem; color: #888; margin-top: 2px; }
    .toggle  { width: 52px; height: 28px; background: #333; border-radius: 14px; cursor: pointer; position: relative; transition: background 0.2s; border: none; }
    .toggle.on { background: #e94560; }
    .toggle::after { content: ''; position: absolute; width: 22px; height: 22px; background: white; border-radius: 50%; top: 3px; left: 3px; transition: left 0.2s; }
    .toggle.on::after { left: 27px; }
    .status { padding: 8px 16px; background: #0f3460; border-radius: 20px; font-size: 0.8rem; text-align: center; margin-bottom: 16px; }
    .btn { display: block; width: 100%; padding: 12px; background: #e94560; border: none; border-radius: 8px; color: white; font-size: 1rem; cursor: pointer; margin-top: 8px; }
    .btn:hover { background: #c73652; }
  </style>
</head>
<body>
  <div class="header">
    <h1 id="device-name">Smart Home</h1>
    <p id="device-ip">Đang tải...</p>
  </div>
  <div class="container">
    <div class="status" id="status">Đang kết nối...</div>
    <div class="card" id="dp-list">Đang tải Data Points...</div>
    <div class="card">
      <button class="btn" onclick="restart()">Khởi động lại</button>
    </div>
  </div>
  <script>
    async function fetchInfo() {
      const r = await fetch('/api/info'); const d = await r.json();
      document.getElementById('device-name').textContent = d.product_name;
      document.getElementById('device-ip').textContent = 'IP: ' + d.ip + ' | v' + d.version;
      document.getElementById('status').textContent = 'Kết nối thành công ✓';
    }
    async function fetchDp() {
      const r = await fetch('/api/dp'); const d = await r.json();
      const list = document.getElementById('dp-list');
      list.innerHTML = Object.entries(d).map(([id, val]) => `
        <div class="dp-row">
          <div><div class="dp-name">DP ${id}</div><div class="dp-val">${JSON.stringify(val)}</div></div>
          ${typeof val === 'boolean' ? `<button class="toggle ${val?'on':''}" onclick="setDp(${id}, ${!val})"></button>` : ''}
        </div>
      `).join('');
    }
    async function setDp(id, value) {
      await fetch('/api/dp/' + id, { method: 'POST', headers: {'Content-Type':'application/json'}, body: JSON.stringify({value}) });
      fetchDp();
    }
    async function restart() {
      if (confirm('Khởi động lại thiết bị?')) {
        await fetch('/api/restart', { method: 'POST' });
      }
    }
    fetchInfo(); fetchDp();
    setInterval(fetchDp, 5000);
  </script>
</body>
</html>"#;

pub struct HttpServer {
    _server: EspHttpServer<'static>,
}

impl HttpServer {
    pub fn new(
        dp_manager: Arc<Mutex<DpManager>>,
        product_name: &'static str,
        product_version: &'static str,
        ip: std::net::Ipv4Addr,
    ) -> anyhow::Result<Self> {
        let config = HttpConfig {
            stack_size: 8192,
            ..Default::default()
        };

        let mut server = EspHttpServer::new(&config)?;

        // GET / → Web UI
        server.fn_handler("/", esp_idf_svc::http::Method::Get, |req| {
            req.into_ok_response()?
                .write_all(INDEX_HTML.as_bytes())?;
            Ok::<(), anyhow::Error>(())
        })?;

        // GET /api/info → Device info JSON
        let ip_clone = ip;
        server.fn_handler("/api/info", esp_idf_svc::http::Method::Get, move |req| {
            let body = serde_json::json!({
                "product_name": product_name,
                "version": product_version,
                "ip": ip_clone.to_string(),
            }).to_string();

            let mut resp = req.into_ok_response()?;
            resp.write_all(body.as_bytes())?;
            Ok::<(), anyhow::Error>(())
        })?;

        // GET /api/dp → Full DP state
        let dp_clone = dp_manager.clone();
        server.fn_handler("/api/dp", esp_idf_svc::http::Method::Get, move |req| {
            let snapshot = dp_clone.lock().unwrap().snapshot();
            let body = codec::encode_snapshot(&snapshot).to_string();
            req.into_ok_response()?.write_all(body.as_bytes())?;
            Ok::<(), anyhow::Error>(())
        })?;

        // POST /api/restart
        server.fn_handler("/api/restart", esp_idf_svc::http::Method::Post, |req| {
            req.into_ok_response()?.write_all(b"{\"ok\":true}")?;
            // Delay rồi restart
            std::thread::spawn(|| {
                std::thread::sleep(std::time::Duration::from_millis(500));
                unsafe { esp_idf_sys::esp_restart() };
            });
            Ok::<(), anyhow::Error>(())
        })?;

        info!("[HTTP] Server started on port 80");
        Ok(Self { _server: server })
    }
}
