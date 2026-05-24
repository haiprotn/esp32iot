# Smart Home Firmware — Rust + ESP-IDF cho ESP32-C3

Firmware IoT điện dân dụng (công tắc, ổ cắm, dimmer) chạy trên ESP32-C3.

## Yêu cầu môi trường (WSL)

```bash
# Kiểm tra đã có đủ chưa
rustup show                  # Cần rust stable
espflash --version           # Tool flash firmware
cargo espflash --version     # Cargo subcommand
```

Nếu chưa đủ, chạy:

```bash
# Cài target RISC-V cho ESP32-C3
rustup target add riscv32imc-esp-espidf

# Cài ESP tools
cargo install espup espflash cargo-espflash

# Setup ESP-IDF toolchain (tải ~2GB lần đầu)
espup install

# Thêm vào .bashrc
echo 'source ~/export-esp.sh' >> ~/.bashrc
source ~/export-esp.sh
```

---

## Build

```bash
cd smart-home-firmware

# Build Smart Switch 1 Gang (default)
cargo build --release

# Build các sản phẩm khác
cargo build --release --features switch_2g
cargo build --release --features switch_3g
cargo build --release --features smart_plug

# Xem binary size
ls -lh target/riscv32imc-esp-espidf/release/smart-home-firmware
```

---

## Flash lên board

### Qua USB (WSL → Windows → ESP32)

> **Lưu ý:** Project dùng OTA partition table, nên phải flash thủ công 4 thành phần riêng biệt. `espflash flash` sẽ ghi sai địa chỉ với OTA layout này.

```bash
# Bước 1: Build firmware
cargo build --release --features switch_1g

# Bước 2: Convert ELF → ESP image binary
ESPTOOL=~/.espressif/python_env/idf5.2_py3.10_env/bin/esptool.py
$ESPTOOL --chip esp32c3 elf2image --flash_mode dio --flash_freq 80m --flash_size 4MB \
  -o /tmp/app.bin \
  target/riscv32imc-esp-espidf/release/smart-home-firmware

# Bước 3: Flash tất cả lên board
BUILD=target/riscv32imc-esp-espidf/release
$ESPTOOL --chip esp32c3 --port /dev/ttyACM0 --baud 921600 write_flash \
  0x0      $BUILD/bootloader.bin \
  0x8000   $BUILD/partition-table.bin \
  0x10000  $BUILD/build/esp-idf-sys-*/out/build/ota_data_initial.bin \
  0x20000  /tmp/app.bin

# Monitor serial log
python3 -c "
import serial, time, re
s = serial.Serial('/dev/ttyACM0', 115200, timeout=1)
s.setDTR(False); s.setRTS(True); time.sleep(0.1); s.setRTS(False)
deadline = time.time() + 30
while time.time() < deadline:
    line = s.readline()
    if line:
        print(re.sub(r'\x1b\[[0-9;]*m', '', line.decode('utf-8', errors='replace')).rstrip())
s.close()
"
```

> **Lưu ý WSL2:** USB device cần được forward vào WSL qua `usbipd`:
> ```powershell
> # Trong PowerShell (Windows) — chạy 1 lần
> winget install usbipd
> usbipd list
> usbipd bind --busid <BUSID>
> usbipd attach --wsl --busid <BUSID>
> ```

---

## GPIO Mapping — Switch 1 Gang

| Chức năng | GPIO | Ghi chú |
|-----------|------|---------|
| Relay     | 4    | → S8050 transistor → Relay coil |
| Button    | 5    | Active LOW, pull-up nội |
| LED       | 8    | Active HIGH |

---

## Cấu trúc Project

```
smart-home-firmware/
├── Cargo.toml              ← Dependencies & features
├── build.rs                ← ESP-IDF build integration
├── sdkconfig.defaults      ← ESP-IDF config chung
├── sdkconfig.defaults.esp32c3  ← Config riêng ESP32-C3
├── partitions.csv          ← Partition table (4MB + OTA)
└── src/
    ├── main.rs             ← Entry point
    ├── config.rs           ← ProductConfig cho từng sản phẩm
    ├── dp/                 ← Data Point system (Tuya-style)
    │   ├── types.rs        ← DpType, DpValue, DpDefinition
    │   ├── manager.rs      ← State + callbacks
    │   └── codec.rs        ← JSON encode/decode
    ├── drivers/            ← Hardware abstraction
    │   ├── relay.rs        ← Relay on/off + rate limiting
    │   ├── button.rs       ← Debounce + short/long press
    │   └── led.rs          ← LED blink patterns
    ├── network/            ← Network services
    │   ├── wifi.rs         ← Connect + auto-reconnect
    │   ├── http_server.rs  ← REST API + Web UI
    │   └── mdns.rs         ← device.local discovery
    ├── provisioning/       ← WiFi setup
    │   └── softap_prov.rs  ← SoftAP + captive portal
    ├── storage/            ← NVS persistent storage
    │   └── mod.rs          ← WiFi creds + DP state
    └── ota/                ← OTA update (Giai đoạn 2)
        └── mod.rs
```

---

## API HTTP (Giai đoạn 1 — Local)

Sau khi kết nối WiFi, thiết bị expose REST API trên port 80:

| Method | Endpoint | Mô tả |
|--------|----------|-------|
| GET | `/` | Web UI điều khiển |
| GET | `/api/info` | Thông tin thiết bị |
| GET | `/api/dp` | Toàn bộ DP state (JSON) |
| POST | `/api/dp/{id}` | Set giá trị DP |
| POST | `/api/restart` | Khởi động lại |

Ví dụ điều khiển relay:
```bash
# Bật relay 1
curl -X POST http://192.168.1.100/api/dp/1 \
  -H "Content-Type: application/json" \
  -d '{"value": true}'

# Tắt relay 1
curl -X POST http://192.168.1.100/api/dp/1 \
  -d '{"value": false}'

# Đặt countdown 300 giây (5 phút)
curl -X POST http://192.168.1.100/api/dp/2 \
  -d '{"value": 300}'
```

---

## Provisioning WiFi

Lần đầu (hoặc sau reset WiFi):
1. Thiết bị bật SoftAP: SSID **`SmartHome-XXXXXX`**
2. Kết nối điện thoại vào AP đó
3. Mở browser → `http://192.168.4.1`
4. Nhập SSID + password → Gửi
5. Thiết bị lưu vào NVS và tự restart

**Reset WiFi:** Giữ nút 3 giây → LED nhấp nháy nhanh → WiFi credentials bị xóa → restart vào provisioning mode.

---

## Lộ trình Phát triển

- [x] **Giai đoạn 1** — Local Control (file này)
- [ ] **Giai đoạn 2** — Remote via MQTT + Django backend
- [ ] **Giai đoạn 3** — Multi-device Dashboard
- [ ] **Giai đoạn 4** — User system + phân quyền
- [ ] **Giai đoạn 5** — Flutter mobile app

---

*Firmware v1.0.0 | ESP32-C3 | Rust + ESP-IDF | 2026*
