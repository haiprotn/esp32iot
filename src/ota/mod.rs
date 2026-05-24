// src/ota/mod.rs — OTA update (Giai đoạn 2)
// Placeholder — sẽ implement khi có server backend

use log::info;

/// Kiểm tra firmware mới từ server
/// url: HTTP endpoint trả về JSON {"version": "x.y.z", "url": "http://..."}
pub async fn check_update(_server_url: &str) -> anyhow::Result<Option<String>> {
    // TODO: Implement khi có backend server (Giai đoạn 2)
    // 1. GET {server_url}/firmware/latest?product=switch_1g&version={current}
    // 2. Parse response
    // 3. So sánh version
    // 4. Nếu có bản mới, trả về URL download
    info!("[OTA] Check update — chưa implement (Giai đoạn 2)");
    Ok(None)
}

/// Thực hiện OTA update từ URL
pub async fn perform_update(_firmware_url: &str) -> anyhow::Result<()> {
    // TODO: Dùng esp_idf_svc::ota::EspOta
    // 1. Tạo OTA session
    // 2. Stream download firmware
    // 3. Flash vào partition dự phòng
    // 4. Verify
    // 5. Set boot partition
    // 6. Restart
    Err(anyhow::anyhow!("OTA chưa implement — xem Giai đoạn 2"))
}
