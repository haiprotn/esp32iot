// src/drivers/led.rs — Status LED driver với các blink patterns

use esp_idf_hal::gpio::{AnyOutputPin, Output, PinDriver};
use esp_idf_hal::peripheral::Peripheral;

/// Các pattern LED biểu thị trạng thái thiết bị
/// Mỗi tick = 100ms (gọi tick() mỗi 100ms trong main loop)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LedPattern {
    /// Tắt — relay OFF, hoạt động bình thường
    Off,
    /// Sáng liên tục — relay ON
    SolidOn,
    /// Nhấp nháy chậm 1Hz — đang kết nối WiFi
    SlowBlink,
    /// Nhấp nháy nhanh 5Hz — đang provisioning (chờ cấu hình WiFi)
    FastBlink,
    /// Nhấp nháy 3 lần rồi tắt — xác nhận thao tác thành công
    TripleBlink,
    /// Nhấp nháy 2 lần nhanh lặp lại — lỗi / mất kết nối
    DoublePulse,
}

pub struct LedDriver<'a> {
    pin: PinDriver<'a, AnyOutputPin, Output>,
    pattern: LedPattern,
    tick: u32,
}

impl<'a> LedDriver<'a> {
    pub fn new(
        pin: impl Peripheral<P = impl esp_idf_hal::gpio::OutputPin> + 'a,
    ) -> anyhow::Result<Self> {
        let mut driver = PinDriver::output(pin.into_ref().map_into())?;
        driver.set_low()?;
        log::info!("[LED] Initialized → OFF");
        Ok(Self {
            pin: driver,
            pattern: LedPattern::Off,
            tick: 0,
        })
    }

    /// Thay đổi pattern — reset counter
    pub fn set_pattern(&mut self, pattern: LedPattern) {
        if self.pattern != pattern {
            log::debug!("[LED] Pattern → {:?}", pattern);
            self.pattern = pattern;
            self.tick = 0;
        }
    }

    /// Gọi mỗi 100ms trong main loop
    pub fn tick(&mut self) -> anyhow::Result<()> {
        self.tick = self.tick.wrapping_add(1);

        match self.pattern {
            LedPattern::Off => {
                self.pin.set_low()?;
            }

            LedPattern::SolidOn => {
                self.pin.set_high()?;
            }

            LedPattern::SlowBlink => {
                // 1Hz: ON 500ms, OFF 500ms (5 ticks mỗi trạng thái)
                if self.tick % 10 < 5 {
                    self.pin.set_high()?;
                } else {
                    self.pin.set_low()?;
                }
            }

            LedPattern::FastBlink => {
                // 5Hz: ON 100ms, OFF 100ms (toggle mỗi tick)
                if self.tick % 2 == 0 {
                    self.pin.set_high()?;
                } else {
                    self.pin.set_low()?;
                }
            }

            LedPattern::TripleBlink => {
                // 3 lần nhấp nháy (mỗi lần 200ms ON + 200ms OFF) rồi tắt hẳn
                // Tổng: 6 tick × 100ms = 600ms active, sau đó OFF
                match self.tick {
                    1 | 3 | 5 => self.pin.set_high()?,
                    2 | 4 | 6 => self.pin.set_low()?,
                    _ => {
                        self.pin.set_low()?;
                        self.pattern = LedPattern::Off;
                    }
                }
            }

            LedPattern::DoublePulse => {
                // 2 lần nháy nhanh rồi nghỉ dài (lặp mỗi 1 giây = 10 ticks)
                match self.tick % 10 {
                    0 | 2 => self.pin.set_high()?,
                    1 | 3 => self.pin.set_low()?,
                    _     => self.pin.set_low()?,
                }
            }
        }
        Ok(())
    }

    /// Pattern hiện tại
    pub fn current_pattern(&self) -> LedPattern {
        self.pattern
    }
}
