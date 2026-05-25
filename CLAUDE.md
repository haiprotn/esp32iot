# CLAUDE.md — Smart Home IoT Project

Hướng dẫn cho Claude Code khi làm việc trong repo này.

## Tổng quan dự án

ESP32-C3 smart home firmware (Rust) + FastAPI backend, triển khai trên WSL2 + Docker.

| Thành phần | Đường dẫn | Công nghệ |
|---|---|---|
| Firmware | `smart-home-firmware/` | Rust, esp-idf-svc 0.50, ESP-IDF v5.2.2 |
| Backend | `/home/haiprotn/smarthome-backend/` | FastAPI, PostgreSQL, Mosquitto, Docker |

**Device hiện tại:** ESP32-C3, MAC `a4:cb:8f:20:d6:c8`, IP LAN `192.168.1.107`
**Firmware version:** v1.0.5
**Backend URL (từ device):** `http://192.168.1.254:8080` (Windows portproxy → WSL2:8000)
**MQTT broker (từ device):** `mqtt://192.168.1.254:1883` (Windows portproxy → WSL2:1883)

---

## Quy tắc làm việc

- **Sau mỗi lần sửa code/config → phải cập nhật README.md** tương ứng.
- Backend dùng **FastAPI** (không phải Django).
- Firmware build: `cargo build --release` trong `smart-home-firmware/`.
- Flash: `espflash flash --port /dev/ttyACM0 target/riscv32imc-esp-espidf/release/smart-home-firmware`
- Monitor serial: dùng Python pyserial (espflash monitor bị lỗi trong WSL2).

---

## Kiến trúc MQTT

```
ESP32 ──► 192.168.1.254:1883 ──► Windows portproxy ──► WSL2:1883 ──► Mosquitto (Docker)
                                                                            │
                                                                      aiomqtt listener
                                                                            │
                                                                       FastAPI backend
```

**Topics:**
- `smarthome/{device_id}/cmd` — backend → device (lệnh điều khiển)
- `smarthome/{device_id}/state` — device → backend (trạng thái DP)
- `smarthome/{device_id}/online` — device publish khi kết nối
- `smarthome/{device_id}/lwt` — LWT "offline" khi mất kết nối

**Quan trọng — MQTT deadlock fix (v1.0.5):**
`subscribe()` và `publish()` KHÔNG được gọi bên trong Connected event callback của `EspMqttClient`.
MQTT internal task bị blocked trong thời gian callback chạy → deadlock → không có PINGREQ → timeout 45s.
Fix: dùng `AtomicBool` flag + dedicated worker thread để subscribe/publish SAU khi event callback đã return.

---

## Network (WSL2 + Windows)

Windows portproxy cần thiết lập thủ công:
```powershell
netsh interface portproxy add v4tov4 listenport=8080 listenaddress=0.0.0.0 connectport=8000 connectaddress=127.0.0.1
netsh interface portproxy add v4tov4 listenport=1883 listenaddress=0.0.0.0 connectport=1883 connectaddress=127.0.0.1
New-NetFirewallRule -DisplayName "SmartHome-8080" -Direction Inbound -Protocol TCP -LocalPort 8080 -Action Allow
New-NetFirewallRule -DisplayName "SmartHome-1883" -Direction Inbound -Protocol TCP -LocalPort 1883 -Action Allow
```

WSL2 IP của Windows host: `192.168.1.254` (thay đổi nếu đổi mạng).

---

## Backend — lệnh thường dùng

```bash
cd /home/haiprotn/smarthome-backend
docker compose up -d          # Khởi động
docker compose logs -f        # Xem log
docker compose restart backend # Reload sau khi sửa Python
```

Mosquitto passwd file: `/home/haiprotn/smarthome-backend/mosquitto/config/passwd`
Format hash: `$7$<iterations>$<base64_salt>$<base64_key>` (PBKDF2-SHA512, iterations=101)
Backend tự tạo user khi device register — không cần chạy `mosquitto_passwd` thủ công.

---

## Firmware — lệnh thường dùng

```bash
cd /home/haiprotn/projects_iot/smart-home-firmware
cargo build --release
espflash flash --port /dev/ttyACM0 target/riscv32imc-esp-espidf/release/smart-home-firmware

# Tạo OTA binary
espflash save-image --chip esp32c3 target/riscv32imc-esp-espidf/release/smart-home-firmware /tmp/switch_1g_vX.Y.Z.bin

# Upload lên backend
docker compose -f /home/haiprotn/smarthome-backend/docker-compose.yml cp \
  /tmp/switch_1g_vX.Y.Z.bin backend:/app/firmware/switch_1g_vX.Y.Z.bin
```

**Bump version trước khi build:** sửa `version` trong `Cargo.toml`.

---

## Tiến trình dự án

- [x] **Giai đoạn 1** — Local Control (firmware GPIO, HTTP API, SoftAP provisioning) — 24/05/2026
- [x] **Giai đoạn 2** — Remote via MQTT + FastAPI backend — 25/05/2026
- [ ] **Giai đoạn 3** — Multi-device Dashboard (web UI nhiều device, rooms)
- [ ] **Giai đoạn 4** — User system + JWT auth + phân quyền
- [ ] **Giai đoạn 5** — Flutter mobile app
