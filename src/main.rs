// src/main.rs — Entry point firmware Smart Home
//
// Chọn product khi build:
//   cargo build --release --features switch_1g
//   cargo build --release --features switch_2g
//   ...

use std::sync::{Arc, Mutex};
use std::time::Duration;

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::prelude::Peripherals;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::log::EspLogger;
use log::{info, warn, error};

mod config;
mod dp;
mod drivers;
mod network;
mod ota;
mod provisioning;
mod storage;

// ─── Chọn product tại compile-time ──────────────────────────────────────────
#[cfg(feature = "switch_1g")]
use config::SWITCH_1G as PRODUCT;

#[cfg(feature = "switch_2g")]
use config::SWITCH_2G as PRODUCT;

#[cfg(feature = "switch_3g")]
use config::SWITCH_3G as PRODUCT;

#[cfg(feature = "smart_plug")]
use config::SMART_PLUG as PRODUCT;

// ─── Entry point ─────────────────────────────────────────────────────────────
fn main() -> anyhow::Result<()> {
    // Bắt buộc: link ESP-IDF patches và khởi tạo logger
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    info!("╔═══════════════════════════════════════════╗");
    info!("║  {} ", PRODUCT.product_name);
    info!("║  Firmware v{}", PRODUCT.firmware_version);
    info!("║  Product ID: {}", PRODUCT.product_id);
    info!("╚═══════════════════════════════════════════╝");

    // ─── Lấy peripherals ─────────────────────────────────────────────────────
    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs_partition = EspDefaultNvsPartition::take()?;

    // ─── 1. Storage (NVS) ────────────────────────────────────────────────────
    let mut storage = storage::NvsStorage::new(nvs_partition.clone())?;
    info!("[Boot] NVS storage ready");

    // ─── 2. GPIO: LED ────────────────────────────────────────────────────────
    use drivers::{LedDriver, LedPattern};

    // Bật LED ngay khi boot (blink chậm = đang khởi động)
    // NOTE: Trong thực tế cần map GPIO number → peripheral
    // Đây là ví dụ với GPIO8 (LED)
    // let mut led = LedDriver::new(peripherals.pins.gpio8)?;
    // led.set_pattern(LedPattern::SlowBlink);
    info!("[Boot] LED driver ready (GPIO{})", PRODUCT.led_pin);

    // ─── 3. GPIO: Relays ─────────────────────────────────────────────────────
    info!("[Boot] Initializing {} relay(s)...", PRODUCT.relay_pins.len());
    for (i, pin) in PRODUCT.relay_pins.iter().enumerate() {
        info!("[Boot]   Relay {} → GPIO{}", i, pin);
    }

    // ─── 4. GPIO: Buttons ────────────────────────────────────────────────────
    info!("[Boot] Initializing {} button(s)...", PRODUCT.button_pins.len());
    for (i, pin) in PRODUCT.button_pins.iter().enumerate() {
        info!("[Boot]   Button {} → GPIO{}", i, pin);
    }

    // ─── 5. DP Manager ───────────────────────────────────────────────────────
    let dp_manager = Arc::new(Mutex::new(
        dp::DpManager::new(PRODUCT.data_points)
    ));

    // Khôi phục trạng thái relay từ NVS
    for dp in PRODUCT.data_points {
        if matches!(dp.dp_type, dp::DpType::Bool) {
            if let Ok(Some(val)) = storage.get_dp_bool(dp.id) {
                dp_manager.lock().unwrap()
                    .set(dp.id, dp::DpValue::Bool(val))
                    .unwrap_or_else(|e| error!("[Boot] Restore DP {} failed: {:?}", dp.id, e));
                info!("[Boot] DP {} '{}' restored → {}", dp.id, dp.name, val);
            }
        }
    }

    // ─── 6. WiFi / Provisioning ──────────────────────────────────────────────
    let wifi_result = if storage.has_wifi_config() {
        let ssid = storage.get_wifi_ssid()?.unwrap();
        let pass = storage.get_wifi_password()?.unwrap_or_default();

        info!("[Boot] WiFi config found: '{}'", ssid);
        // led.set_pattern(LedPattern::SlowBlink);

        // Kết nối WiFi
        let mut wifi_mgr = network::WifiManager::new(
            peripherals.modem,
            sysloop.clone(),
            nvs_partition.clone(),
        )?;

        match wifi_mgr.connect(&ssid, &pass) {
            Ok(ip) => {
                info!("[Boot] WiFi connected! IP: {}", ip);
                // led.set_pattern(LedPattern::SolidOn);
                Some((wifi_mgr, ip, ssid, pass))
            }
            Err(e) => {
                error!("[Boot] WiFi connect failed: {:?}", e);
                // led.set_pattern(LedPattern::DoublePulse);
                None
            }
        }
    } else {
        info!("[Boot] No WiFi config — starting SoftAP provisioning...");
        // led.set_pattern(LedPattern::FastBlink);

        // Tạo AP SSID duy nhất từ MAC
        let ap_ssid = format!("SmartHome-{}", network::get_mac_suffix());
        info!("[Boot] SoftAP SSID: '{}'", ap_ssid);
        info!("[Boot] Kết nối vào '{}' và mở http://192.168.4.1", ap_ssid);

        // Chờ user cấu hình (blocking)
        // TODO: implement SoftAP flow đầy đủ
        // let prov = provisioning::SoftApProvisioning::start(...)?;
        // loop { if let Some((ssid, pass)) = prov.get_credentials() { ... } }

        None
    };

    // ─── 7. Khởi động services (nếu có WiFi) ─────────────────────────────────
    let _http_server = if let Some((_, ip, _, _)) = &wifi_result {
        // HTTP server
        let http = network::HttpServer::new(
            dp_manager.clone(),
            PRODUCT.product_name,
            PRODUCT.firmware_version,
            *ip,
        )?;
        info!("[Boot] HTTP server: http://{}", ip);

        // mDNS (only when mdns ESP-IDF component is enabled)
        let hostname = network::make_hostname(PRODUCT.product_id);
        #[cfg(any(esp_idf_comp_mdns_enabled, esp_idf_comp_espressif__mdns_enabled))]
        match network::MdnsService::new(&hostname, PRODUCT.product_name) {
            Ok(_) => info!("[Boot] mDNS: http://{}.local", hostname),
            Err(e) => warn!("[Boot] mDNS failed: {:?}", e),
        }
        #[cfg(not(any(esp_idf_comp_mdns_enabled, esp_idf_comp_espressif__mdns_enabled)))]
        info!("[Boot] mDNS disabled (component not in build)");

        Some(http)
    } else {
        None
    };

    info!("[Boot] ══ System ready ══");

    // ─── 8. Main loop ─────────────────────────────────────────────────────────
    // Tick counters
    let mut tick_10ms: u32 = 0;   // Tăng mỗi 10ms
    let mut tick_100ms: u32 = 0;  // Tăng mỗi 100ms (LED tick)
    let mut tick_30s: u32 = 0;    // Tăng mỗi 30 giây (WiFi check)

    loop {
        std::thread::sleep(Duration::from_millis(10));
        tick_10ms = tick_10ms.wrapping_add(1);

        // ── Poll buttons (mỗi 10ms) ──────────────────────────────────────────
        // Trong thực tế:
        // for (i, btn) in buttons.iter_mut().enumerate() {
        //     match btn.poll() {
        //         Some(ButtonEvent::ShortPress) => {
        //             let new_state = relays[i].toggle()?;
        //             dp_manager.lock().unwrap().set(i as u8 + 1, DpValue::Bool(new_state))?;
        //             storage.save_dp_bool(i as u8 + 1, new_state)?;
        //             led.set_pattern(LedPattern::TripleBlink);
        //         }
        //         Some(ButtonEvent::LongPress) => {
        //             // Reset WiFi
        //             storage.clear_wifi()?;
        //             esp_idf_svc::sys::esp_restart();
        //         }
        //         None => {}
        //     }
        // }

        // ── LED tick (mỗi 100ms = 10 × 10ms) ───────────────────────────────
        if tick_10ms % 10 == 0 {
            tick_100ms = tick_100ms.wrapping_add(1);
            // led.tick()?;

            // Kiểm tra countdown timers
            // check_countdowns(&mut dp_manager, &mut relays, &mut storage)?;
        }

        // ── WiFi health check (mỗi 30 giây = 3000 × 10ms) ──────────────────
        if tick_10ms % 3000 == 0 {
            tick_30s = tick_30s.wrapping_add(1);
            // TODO: wifi_mgr.ensure_connected(ssid, pass)?;
            let _ = tick_30s;
        }

        // ── Feed watchdog ────────────────────────────────────────────────────
        // esp_idf_sys::esp_task_wdt_reset() nếu cần
    }
}

/// Helper: kiểm tra và thực thi countdown timers
#[allow(dead_code)]
fn check_countdowns(
    dp_manager: &Arc<Mutex<dp::DpManager>>,
) {
    // TODO: Với mỗi relay có countdown DP:
    // 1. Đọc countdown value
    // 2. Nếu countdown > 0, giảm dần theo thời gian
    // 3. Khi countdown = 0, tắt relay tương ứng
}
