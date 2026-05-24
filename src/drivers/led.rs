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
    active_low: bool,
}

impl<'a> LedDriver<'a> {
    pub fn new(
        pin: impl Peripheral<P = impl esp_idf_hal::gpio::OutputPin> + 'a,
        active_low: bool,
    ) -> anyhow::Result<Self> {
        let mut driver = PinDriver::output(pin.into_ref().map_into())?;
        // OFF state: active_low → set_high, active_high → set_low
        if active_low { driver.set_high()? } else { driver.set_low()? }
        log::info!("[LED] Initialized → OFF (active_{})", if active_low { "low" } else { "high" });
        Ok(Self {
            pin: driver,
            pattern: LedPattern::Off,
            tick: 0,
            active_low,
        })
    }

    fn write(&mut self, on: bool) -> anyhow::Result<()> {
        let pin_high = if self.active_low { !on } else { on };
        if pin_high { self.pin.set_high()? } else { self.pin.set_low()? }
        Ok(())
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
                self.write(false)?;
            }

            LedPattern::SolidOn => {
                self.write(true)?;
            }

            LedPattern::SlowBlink => {
                self.write(self.tick % 10 < 5)?;
            }

            LedPattern::FastBlink => {
                self.write(self.tick % 2 == 0)?;
            }

            LedPattern::TripleBlink => {
                match self.tick {
                    1 | 3 | 5 => self.write(true)?,
                    2 | 4 | 6 => self.write(false)?,
                    _ => {
                        self.write(false)?;
                        self.pattern = LedPattern::Off;
                    }
                }
            }

            LedPattern::DoublePulse => {
                match self.tick % 10 {
                    0 | 2 => self.write(true)?,
                    _     => self.write(false)?,
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
