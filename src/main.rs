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
    use drivers::{ButtonDriver, ButtonEvent, LedDriver, LedPattern};

    let mut led = LedDriver::new(peripherals.pins.gpio8)?;
    led.set_pattern(LedPattern::SlowBlink);
    info!("[Boot] LED driver ready (GPIO{})", PRODUCT.led_pin);

    // ─── 3. GPIO: Relays ─────────────────────────────────────────────────────
    #[cfg(feature = "switch_1g")]
    let mut relay0 = drivers::RelayDriver::new(peripherals.pins.gpio4, 0)?;
    #[cfg(feature = "switch_2g")]
    let (mut relay0, mut relay1) = (
        drivers::RelayDriver::new(peripherals.pins.gpio4, 0)?,
        drivers::RelayDriver::new(peripherals.pins.gpio5, 1)?,
    );
    #[cfg(feature = "switch_3g")]
    let (mut relay0, mut relay1, mut relay2) = (
        drivers::RelayDriver::new(peripherals.pins.gpio4, 0)?,
        drivers::RelayDriver::new(peripherals.pins.gpio5, 1)?,
        drivers::RelayDriver::new(peripherals.pins.gpio6, 2)?,
    );
    #[cfg(feature = "smart_plug")]
    let mut relay0 = drivers::RelayDriver::new(peripherals.pins.gpio4, 0)?;

    // Khôi phục relay state từ NVS → cập nhật LED
    #[cfg(any(feature = "switch_1g", feature = "smart_plug"))]
    if let Ok(Some(val)) = storage.get_dp_bool(1) {
        relay0.restore_state(val)?;
        if val { led.set_pattern(LedPattern::SolidOn); } else { led.set_pattern(LedPattern::Off); }
    }

    // ─── 4. GPIO: Buttons ────────────────────────────────────────────────────
    #[cfg(feature = "switch_1g")]
    let mut btn0 = ButtonDriver::new(peripherals.pins.gpio9, 0)?;
    #[cfg(feature = "switch_2g")]
    let (mut btn0, mut btn1) = (
        ButtonDriver::new(peripherals.pins.gpio6, 0)?,
        ButtonDriver::new(peripherals.pins.gpio7, 1)?,
    );
    #[cfg(feature = "switch_3g")]
    let (mut btn0, mut btn1, mut btn2) = (
        ButtonDriver::new(peripherals.pins.gpio7, 0)?,
        ButtonDriver::new(peripherals.pins.gpio9, 1)?,
        ButtonDriver::new(peripherals.pins.gpio10, 2)?,
    );
    #[cfg(feature = "smart_plug")]
    let mut btn0 = ButtonDriver::new(peripherals.pins.gpio5, 0)?;

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
        led.set_pattern(LedPattern::SlowBlink);

        // Kết nối WiFi
        let mut wifi_mgr = network::WifiManager::new(
            peripherals.modem,
            sysloop.clone(),
            nvs_partition.clone(),
        )?;

        match wifi_mgr.connect(&ssid, &pass) {
            Ok(ip) => {
                info!("[Boot] WiFi connected! IP: {}", ip);
                led.set_pattern(LedPattern::SlowBlink); // relay state sẽ set lại bên dưới
                Some((wifi_mgr, ip, ssid, pass))
            }
            Err(e) => {
                error!("[Boot] WiFi connect failed: {:?}", e);
                led.set_pattern(LedPattern::DoublePulse);
                None
            }
        }
    } else {
        info!("[Boot] No WiFi config — starting SoftAP provisioning...");
        led.set_pattern(LedPattern::FastBlink);

        let ap_ssid = format!("SmartHome-{}", network::get_mac_suffix());
        info!("[Boot] SoftAP SSID: '{}'", ap_ssid);
        info!("[Boot] Connect to '{}' and open http://192.168.4.1", ap_ssid);

        let prov = provisioning::SoftApProvisioning::start(
            peripherals.modem,
            sysloop.clone(),
            nvs_partition.clone(),
            &ap_ssid,
        )?;

        loop {
            if let Some((ssid, pass)) = prov.get_credentials() {
                storage.save_wifi(&ssid, &pass)?;
                info!("[Provision] WiFi saved (SSID: '{}'), restarting...", ssid);
                std::thread::sleep(Duration::from_millis(500));
                unsafe { esp_idf_svc::sys::esp_restart() };
            }
            if prov.is_timed_out() {
                warn!("[Provision] SoftAP timeout, restarting...");
                unsafe { esp_idf_svc::sys::esp_restart() };
            }
            std::thread::sleep(Duration::from_millis(100));
        }
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
        #[cfg(feature = "switch_1g")]
        match btn0.poll() {
            Some(ButtonEvent::ShortPress) => {
                let new_state = relay0.toggle()?;
                dp_manager.lock().unwrap().set(1, dp::DpValue::Bool(new_state)).ok();
                storage.save_dp_bool(1, new_state)?;
                led.set_pattern(if new_state { LedPattern::SolidOn } else { LedPattern::Off });
                info!("[Button] Relay → {}", if new_state { "ON" } else { "OFF" });
            }
            Some(ButtonEvent::LongPress) => {
                warn!("[Button] Long press — reset WiFi, restarting...");
                led.set_pattern(LedPattern::FastBlink);
                storage.clear_wifi()?;
                std::thread::sleep(Duration::from_millis(1000));
                unsafe { esp_idf_svc::sys::esp_restart() };
            }
            None => {}
        }

        // ── LED tick (mỗi 100ms = 10 × 10ms) ───────────────────────────────
        if tick_10ms % 10 == 0 {
            tick_100ms = tick_100ms.wrapping_add(1);
            led.tick()?;

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
