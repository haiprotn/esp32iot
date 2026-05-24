// src/drivers/button.rs — Button driver: debounce + short/long press

use esp_idf_hal::gpio::{AnyInputPin, Input, PinDriver};
use esp_idf_hal::peripheral::Peripheral;
use std::time::Instant;

/// Thời gian debounce (ms) — bỏ qua nhiễu cơ học
const DEBOUNCE_MS: u128 = 50;

/// Thời gian giữ nút để kích hoạt long press (ms)
/// Long press = reset WiFi / vào chế độ provisioning
const LONG_PRESS_MS: u128 = 3000;

/// Event từ button
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonEvent {
    /// Nhấn ngắn — toggle relay / đèn
    ShortPress,
    /// Giữ 3 giây — reset WiFi, vào provisioning mode
    LongPress,
}

pub struct ButtonDriver<'a> {
    pin: PinDriver<'a, AnyInputPin, Input>,
    index: usize,
    press_start: Option<Instant>,
    was_pressed: bool,
    long_press_fired: bool,
}

impl<'a> ButtonDriver<'a> {
    /// Khởi tạo button driver
    /// Giả định nút nối GND → active LOW, cần pull-up nội
    pub fn new(
        pin: impl Peripheral<P = impl esp_idf_hal::gpio::InputPin> + 'a,
        index: usize,
    ) -> anyhow::Result<Self> {
        let driver = PinDriver::input(pin.into_ref().map_into())?;
        // Lưu ý: pull-up được bật qua sdkconfig hoặc cấu hình GPIO riêng
        log::info!("[Button {}] Initialized", index);
        Ok(Self {
            pin: driver,
            index,
            press_start: None,
            was_pressed: false,
            long_press_fired: false,
        })
    }

    /// Poll button state — gọi trong main loop mỗi ~10ms
    /// Trả về Some(event) nếu có sự kiện, None nếu không
    pub fn poll(&mut self) -> Option<ButtonEvent> {
        // Active LOW: is_low() = đang nhấn
        let pressed = self.pin.is_low();

        match (self.was_pressed, pressed) {
            // ── Vừa nhấn xuống ──────────────────────────────────────────────
            (false, true) => {
                self.press_start = Some(Instant::now());
                self.was_pressed = true;
                self.long_press_fired = false;
                log::debug!("[Button {}] Pressed", self.index);
                None
            }

            // ── Đang giữ — kiểm tra long press ──────────────────────────────
            (true, true) => {
                if !self.long_press_fired {
                    if let Some(start) = self.press_start {
                        if start.elapsed().as_millis() >= LONG_PRESS_MS {
                            self.long_press_fired = true;
                            log::info!("[Button {}] Long press detected", self.index);
                            return Some(ButtonEvent::LongPress);
                        }
                    }
                }
                None
            }

            // ── Vừa thả ra ──────────────────────────────────────────────────
            (true, false) => {
                self.was_pressed = false;
                let event = if !self.long_press_fired {
                    // Chỉ tính short press nếu đã qua debounce
                    if let Some(start) = self.press_start.take() {
                        let duration = start.elapsed().as_millis();
                        if duration >= DEBOUNCE_MS {
                            log::debug!("[Button {}] Short press ({} ms)", self.index, duration);
                            Some(ButtonEvent::ShortPress)
                        } else {
                            log::debug!("[Button {}] Ignored (noise, {} ms)", self.index, duration);
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    self.press_start = None;
                    None
                };
                event
            }

            // ── Không nhấn (idle) ────────────────────────────────────────────
            (false, false) => None,
        }
    }

    /// Trả về true nếu nút đang được giữ
    pub fn is_held(&self) -> bool {
        self.was_pressed
    }

    /// Thời gian đã giữ (ms), 0 nếu không nhấn
    pub fn held_duration_ms(&self) -> u128 {
        match (self.was_pressed, self.press_start) {
            (true, Some(start)) => start.elapsed().as_millis(),
            _ => 0,
        }
    }
}
