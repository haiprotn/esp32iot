// src/config.rs — ProductConfig: định nghĩa phần cứng và DP cho từng sản phẩm

use crate::dp::types::{DpDefinition, DpType};

/// Cấu hình đầy đủ cho một sản phẩm — tất cả là &'static để nằm trên ROM
pub struct ProductConfig {
    pub product_id: &'static str,
    pub product_name: &'static str,
    pub firmware_version: &'static str,

    // GPIO mapping
    pub relay_pins: &'static [u8],
    pub button_pins: &'static [u8],
    pub led_pin: u8,

    // Power meter (chỉ dùng cho Smart Plug)
    pub has_power_meter: bool,
    pub power_cf_pin: Option<u8>,
    pub power_cf1_pin: Option<u8>,

    // Data Points theo Tuya protocol
    pub data_points: &'static [DpDefinition],
}

// ─── Smart Switch 1 Gang ─────────────────────────────────────────────────────
// Relay: GPIO4  |  Button: GPIO5  |  LED: GPIO8

pub const SWITCH_1G: ProductConfig = ProductConfig {
    product_id: "switch_1g",
    product_name: "Smart Switch 1 Gang",
    firmware_version: env!("CARGO_PKG_VERSION"),

    relay_pins:  &[4],
    button_pins: &[5],
    led_pin: 8,

    has_power_meter: false,
    power_cf_pin:  None,
    power_cf1_pin: None,

    data_points: &[
        DpDefinition {
            id: 1,
            name: "switch_1",
            dp_type: DpType::Bool,
            writable: true,
        },
        DpDefinition {
            id: 2,
            name: "countdown_1",
            dp_type: DpType::Int { min: 0, max: 86400 },
            writable: true,
        },
    ],
};

// ─── Smart Switch 2 Gang ─────────────────────────────────────────────────────
// Relay: GPIO4, GPIO5  |  Button: GPIO6, GPIO7  |  LED: GPIO8

pub const SWITCH_2G: ProductConfig = ProductConfig {
    product_id: "switch_2g",
    product_name: "Smart Switch 2 Gang",
    firmware_version: env!("CARGO_PKG_VERSION"),

    relay_pins:  &[4, 5],
    button_pins: &[6, 7],
    led_pin: 8,

    has_power_meter: false,
    power_cf_pin:  None,
    power_cf1_pin: None,

    data_points: &[
        DpDefinition { id: 1, name: "switch_1",    dp_type: DpType::Bool, writable: true },
        DpDefinition { id: 2, name: "switch_2",    dp_type: DpType::Bool, writable: true },
        DpDefinition { id: 3, name: "countdown_1", dp_type: DpType::Int { min: 0, max: 86400 }, writable: true },
        DpDefinition { id: 4, name: "countdown_2", dp_type: DpType::Int { min: 0, max: 86400 }, writable: true },
    ],
};

// ─── Smart Switch 3 Gang ─────────────────────────────────────────────────────

pub const SWITCH_3G: ProductConfig = ProductConfig {
    product_id: "switch_3g",
    product_name: "Smart Switch 3 Gang",
    firmware_version: env!("CARGO_PKG_VERSION"),

    relay_pins:  &[4, 5, 6],
    button_pins: &[7, 9, 10],
    led_pin: 8,

    has_power_meter: false,
    power_cf_pin:  None,
    power_cf1_pin: None,

    data_points: &[
        DpDefinition { id: 1, name: "switch_1",    dp_type: DpType::Bool, writable: true },
        DpDefinition { id: 2, name: "switch_2",    dp_type: DpType::Bool, writable: true },
        DpDefinition { id: 3, name: "switch_3",    dp_type: DpType::Bool, writable: true },
        DpDefinition { id: 4, name: "countdown_1", dp_type: DpType::Int { min: 0, max: 86400 }, writable: true },
        DpDefinition { id: 5, name: "countdown_2", dp_type: DpType::Int { min: 0, max: 86400 }, writable: true },
        DpDefinition { id: 6, name: "countdown_3", dp_type: DpType::Int { min: 0, max: 86400 }, writable: true },
    ],
};

// ─── Smart Plug ──────────────────────────────────────────────────────────────

pub const SMART_PLUG: ProductConfig = ProductConfig {
    product_id: "smart_plug",
    product_name: "Smart Plug",
    firmware_version: env!("CARGO_PKG_VERSION"),

    relay_pins:  &[4],
    button_pins: &[5],
    led_pin: 8,

    has_power_meter: true,
    power_cf_pin:  Some(6), // HLW8032 CF  → công suất
    power_cf1_pin: Some(7), // HLW8032 CF1 → dòng/áp

    data_points: &[
        DpDefinition { id: 1, name: "switch",    dp_type: DpType::Bool, writable: true },
        DpDefinition { id: 2, name: "countdown", dp_type: DpType::Int { min: 0, max: 86400 }, writable: true },
        DpDefinition { id: 3, name: "power",     dp_type: DpType::Int { min: 0, max: 50000 }, writable: false }, // W × 10
        DpDefinition { id: 4, name: "voltage",   dp_type: DpType::Int { min: 0, max: 2600 },  writable: false }, // V × 10
        DpDefinition { id: 5, name: "current",   dp_type: DpType::Int { min: 0, max: 30000 }, writable: false }, // mA
        DpDefinition { id: 6, name: "energy",    dp_type: DpType::Int { min: 0, max: i32::MAX }, writable: false }, // Wh
    ],
};
