# Nysa

A lightweight HTTP server for ESP32-WROOM-32 with WiFi provisioning and filesystem OTA updates.

## Prerequisites

**ESP32 uses Xtensa architecture**, requiring a custom Rust toolchain.

### 1. Install Tools

```bash
# Install just (task runner) if you don't have it
cargo install just

# Install all required tools (espup, espflash, ldproxy, littlefs-tools)
just setup
```

### 2. Activate ESP Environment

The ESP environment must be activated in every new terminal:

```bash
# Windows CMD:
%USERPROFILE%\.espup\esp\export-esp.bat

# Windows PowerShell:
$HOME\.espup\esp\export-esp.ps1

# Linux/macOS:
. $HOME/.espup/esp/export-esp.sh
```

### 3. Build the Project

```bash
just build
```

## Quick Start

After setting up the toolchain:

```bash
# Set your ESP32 port
set ESP_PORT=COM3        # Windows
export ESP_PORT=/dev/ttyUSB0  # Linux

# Erase, build, flash, and monitor
just all
```

## Features

- **SoftAP Provisioning**: Creates "Nysa-Setup" network for easy WiFi configuration
- **Static File Serving**: Serves files from LittleFS partition
- **OTA Updates**: HTTP endpoint to update filesystem image remotely
- **Secure**: Compile-time OTA key authentication with input validation
- **Web UI**: Provisioning and status pages with embedded fallbacks

## Security

- **OTA Key**: Set `OTA_KEY` environment variable at build time. The default key is insecure and must be changed for any deployment.
- **Input Validation**: WiFi credentials are validated against ESP32 limits (SSID: 1-32 chars, password: 8-63 chars or empty for open networks).
- **Path Sanitization**: Static file requests are checked for path traversal attempts.
- **Credential Privacy**: WiFi passwords are never written to log output.
- **Error Sanitization**: Internal error details are not exposed in HTTP responses.
- **Auth Required**: The `/api/status`, `DELETE /api/wifi`, and `/ota/fs` endpoints require the `X-OTA-Key` header.

## Project Structure

```
nysa/
├── src/
│   ├── main.rs         # Entry point + LittleFS mount
│   ├── wifi.rs         # WiFi AP/STA management
│   ├── web_server.rs   # HTTP server + routes
│   ├── ota.rs          # Filesystem OTA handler
│   └── storage.rs      # NVS storage for WiFi config
├── nysa-utils/         # Pure utility functions (testable on host)
│   └── src/lib.rs      # Content types, validation, path sanitization
├── static/             # Web files (embedded + served from LittleFS)
│   ├── index.html      # WiFi provisioning page
│   ├── connected.html  # Connected status page
│   └── 404.html        # Error page
├── partitions.csv      # ESP32 partition table
├── build.rs            # Build script (OTA key configuration)
├── Justfile            # Build automation
├── CONFIG.md           # Advanced configuration guide
└── README.md
```

## WiFi Provisioning

1. **First boot**: ESP32 starts "Nysa-Setup" SoftAP
2. **Connect**: Join network (password: `nysa-setup-123`)
3. **Configure**: Visit http://192.168.71.1/
4. **Enter**: Your WiFi credentials
5. **Done**: Device restarts and connects automatically

## Filesystem OTA

```bash
# Create filesystem image from the static/ directory
just create-fs static

# Upload via HTTP
curl -X POST http://<device-ip>/ota/fs \
  -H "X-OTA-Key: your-secret-key" \
  --data-binary @target/littlefs.bin
```

## API Endpoints

| Endpoint | Method | Auth | Description |
|----------|--------|------|-------------|
| `/` | GET | No | WiFi provisioning / status page |
| `/api/status` | GET | Yes | Device status (JSON) |
| `/api/wifi` | POST | No | Configure WiFi |
| `/api/wifi` | DELETE | Yes | Clear WiFi credentials |
| `/ota/fs` | POST | Yes | Upload filesystem image |
| `/status` | GET | No | Status page (direct access) |

## Build Commands

Run `just` with no arguments to see all available commands. Key commands:

```bash
just setup          # Install required tools (one-time)
just build          # Build firmware (release)
just build-key KEY  # Build with custom OTA key
just flash          # Build, flash, and monitor
just all            # Erase, build, flash, and monitor
just create-fs DIR  # Create LittleFS image from directory
just flash-fs       # Flash filesystem image only
just monitor        # Monitor serial output
just erase-nvs      # Erase NVS (clear saved WiFi)
just erase-fs       # Erase LittleFS partition
just clean          # Clean build artifacts
just test-unit      # Run unit tests (no ESP32 needed)
just test           # Full check: fmt + check + clippy + unit tests
```

## Testing

Pure utility functions (content type detection, input validation, path sanitization) live in `nysa-utils/` and can be tested on any machine without ESP32 hardware:

```bash
just test-unit
# or directly (adjust --target for your platform):
cargo +stable test --manifest-path nysa-utils/Cargo.toml --target x86_64-pc-windows-msvc
```

## Troubleshooting

### "can't find crate for core" / target not installed

Run the ESP environment setup:
```bash
# Windows
%USERPROFILE%\.espup\esp\export-esp.bat

# Linux/Mac
. $HOME/.espup/esp/export-esp.sh
```

### "xtensa-esp32-espidf not found"

The Xtensa target requires espup:
```bash
just setup
```

### "No such file or directory" for espflash

Install the tools:
```bash
just setup
```

### WiFi won't connect

- Verify credentials at provisioning page
- Ensure 2.4GHz network (ESP32 doesn't support 5GHz)
- Check signal strength

### OTA upload fails

- Verify OTA_KEY matches between build and request
- Ensure filesystem image size <= 1MB
- Check device has enough free heap memory

## Configuration

### Change OTA Key

Set `OTA_KEY` environment variable before building:
```bash
# Windows
set OTA_KEY=my-super-secret-key
just build-key %OTA_KEY%

# Linux/macOS
export OTA_KEY=my-super-secret-key
just build-key $OTA_KEY
```

### Custom Partition Layout

Edit `partitions.csv` and adjust offsets in code if needed.

### Add Web Files

Place files in `static/` directory, then create and flash the filesystem image:
```bash
just create-fs static
just flash-fs
```

## Hardware Requirements

- **Board**: ESP32-WROOM-32 (4MB flash)
- **Power**: 5V via USB or external supply
- **Flash Size**: 4MB minimum

## Partition Layout (4MB)

| Name | Type | Offset | Size |
|------|------|--------|------|
| nvs | data | 0x9000 | 24KB |
| phy_init | data | 0xF000 | 4KB |
| factory | app | 0x10000 | 1.5MB |
| storage | data | 0x190000 | 1MB |

## License

MIT License

## See Also

- [CONFIG.md](CONFIG.md) - Advanced configuration guide
