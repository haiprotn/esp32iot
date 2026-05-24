// src/drivers/relay.rs — Relay driver với safety checks

use esp_idf_hal::gpio::{AnyOutputPin, Output, PinDriver};
use esp_idf_hal::peripheral::Peripheral;
use log::info;

/// Thời gian tối thiểu giữa 2 lần đóng/ngắt relay (ms)
/// Bảo vệ tiếp điểm relay khỏi đóng cắt quá nhanh
const RELAY_MIN_SWITCH_INTERVAL_MS: u64 = 200;

pub struct RelayDriver<'a> {
    pin: PinDriver<'a, AnyOutputPin, Output>,
    state: bool,
    index: usize,
    last_switch_ms: u64,
}

impl<'a> RelayDriver<'a> {
    /// Khởi tạo relay — mặc định OFF (an toàn khi reset/mất điện)
    pub fn new(
        pin: impl Peripheral<P = impl esp_idf_hal::gpio::OutputPin> + 'a,
        index: usize,
    ) -> anyhow::Result<Self> {
        let mut driver = PinDriver::output(pin.into_ref().map_into())?;
        driver.set_low()?; // SAFETY: Relay OFF khi khởi động
        info!("[Relay {}] Initialized → OFF", index);
        Ok(Self {
            pin: driver,
            state: false,
            index,
            last_switch_ms: 0,
        })
    }

    /// Bật/tắt relay — có kiểm tra tần suất đóng cắt
    pub fn set(&mut self, on: bool) -> anyhow::Result<()> {
        // Kiểm tra xem có đang đổi trạng thái không
        if self.state == on {
            return Ok(()); // Không cần làm gì
        }

        // Rate limiting: tránh đóng/ngắt quá nhanh
        let now_ms = self.millis_now();
        let elapsed = now_ms.saturating_sub(self.last_switch_ms);
        if elapsed < RELAY_MIN_SWITCH_INTERVAL_MS {
            log::warn!(
                "[Relay {}] Bỏ qua lệnh — đóng/ngắt quá nhanh ({} ms < {} ms)",
                self.index, elapsed, RELAY_MIN_SWITCH_INTERVAL_MS
            );
            return Ok(());
        }

        if on {
            self.pin.set_high()?;
        } else {
            self.pin.set_low()?;
        }

        self.state = on;
        self.last_switch_ms = now_ms;
        info!("[Relay {}] → {}", self.index, if on { "ON" } else { "OFF" });
        Ok(())
    }

    /// Toggle trạng thái hiện tại
    /// Trả về trạng thái mới sau khi toggle
    pub fn toggle(&mut self) -> anyhow::Result<bool> {
        let new_state = !self.state;
        self.set(new_state)?;
        Ok(self.state)
    }

    /// Trạng thái hiện tại
    pub fn is_on(&self) -> bool {
        self.state
    }

    /// Cập nhật state từ NVS (sau khi khôi phục từ bộ nhớ) — không rate limit
    pub fn restore_state(&mut self, on: bool) -> anyhow::Result<()> {
        if on {
            self.pin.set_high()?;
        } else {
            self.pin.set_low()?;
        }
        self.state = on;
        info!("[Relay {}] State restored → {}", self.index, if on { "ON" } else { "OFF" });
        Ok(())
    }

    fn millis_now(&self) -> u64 {
        // esp_idf_svc::systime::EspSystemTime không cần khởi tạo
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}
