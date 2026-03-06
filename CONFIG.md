# Configuration Guide

## Environment Variables

### OTA_KEY
Security key for filesystem OTA updates. Must be provided in the `X-OTA-Key` HTTP header.

```bash
# Windows
set OTA_KEY=your-super-secret-key
just build-key %OTA_KEY%

# Linux/macOS
export OTA_KEY=your-super-secret-key
just build-key $OTA_KEY
```

**Default:** `change-me-in-production`

**Important:** Change this for production deployments!

### ESP_PORT
Serial port for ESP32 communication.

```bash
# Windows
set ESP_PORT=COM3

# Linux
export ESP_PORT=/dev/ttyUSB0

# macOS
export ESP_PORT=/dev/cu.usbserial-0001
```

**Default:** `COM3` (Windows), `/dev/ttyUSB0` (Linux)

## Partition Table

The default partition layout for 4MB ESP32-WROOM-32:

| Name     | Type  | Offset   | Size    | Purpose              |
|----------|-------|----------|---------|---------------------|
| nvs      | data  | 0x9000   | 24KB    | NVS storage         |
| phy_init | data  | 0xF000   | 4KB     | PHY init data       |
| factory  | app   | 0x10000  | 1.5MB   | Firmware            |
| storage  | data  | 0x190000 | 1MB     | LittleFS filesystem |

To customize, edit `partitions.csv` and update the `LITTLEFS_MOUNT_POINT` constant in `src/main.rs` if needed.

## WiFi Provisioning

### SoftAP Mode
- **SSID:** `Nysa-Setup`
- **Password:** `nysa-setup-123`
- **IP Address:** `192.168.71.1`

### Changing Credentials
Connect to the SoftAP and visit `http://192.168.71.1/` to configure WiFi.

To reset credentials:
1. Visit `http://<device-ip>/status` and click "Forget WiFi"
2. Or send a DELETE request with the OTA key:
   ```bash
   curl -X DELETE http://<device-ip>/api/wifi -H "X-OTA-Key: your-key"
   ```
3. Or erase NVS via serial: `just erase-nvs`

## OTA Updates

### Filesystem OTA
Upload a new LittleFS image:

```bash
curl -X POST http://<device-ip>/ota/fs \
  -H "X-OTA-Key: your-key" \
  --data-binary @target/littlefs.bin
```

Or use the Justfile:
```bash
just create-fs static  # Create LittleFS image from static/ directory
just flash-fs           # Flash image to ESP32 via serial
```

### Firmware OTA
(Not yet implemented - would require OTA partition)

## API Endpoints

### GET /api/status
Returns device status information. Requires `X-OTA-Key` header.

**Response:**
```json
{
  "connected": true,
  "ssid": "MyWiFi",
  "ip": "192.168.1.100",
  "uptime": 3600
}
```

### POST /api/wifi
Configure WiFi credentials.

**Request:**
```json
{
  "ssid": "MyWiFi",
  "password": "secret123"
}
```

**Validation:** SSID must be 1-32 characters. Password must be 8-63 characters (or empty for open networks).

### DELETE /api/wifi
Clear WiFi credentials and reboot into provisioning mode. Requires `X-OTA-Key` header.

### POST /ota/fs
Upload new filesystem image. Requires `X-OTA-Key` header.

## Static Files

Place web files in the `static/` directory. Files are served from LittleFS at runtime.

**Default files:**
- `index.html` - WiFi provisioning page
- `connected.html` - Status page when connected
- `404.html` - Error page

**Custom files:** Add any HTML, CSS, JS, or images to `static/`, then create and flash the filesystem image:
```bash
just create-fs static   # Create LittleFS image
just flash-fs            # Flash to ESP32 via serial
# Or upload via OTA:
curl -X POST http://<device-ip>/ota/fs -H "X-OTA-Key: your-key" --data-binary @target/littlefs.bin
```

## Security Considerations

1. **Change OTA_KEY** — The default key (`change-me-in-production`) provides no security. Use a strong, unique key of 32+ characters.
2. **Credential Privacy** — WiFi passwords are never logged. Serial output does not contain sensitive credentials.
3. **Input Validation** — WiFi SSID and password are validated before storage:
   - SSID: 1-32 characters (non-empty, within ESP32 limits)
   - Password: 8-63 characters (WPA2), or empty for open networks
4. **Path Traversal** — Static file requests are sanitized to prevent directory traversal attacks (`..` sequences are rejected).
5. **Timing Attacks** — OTA key comparison uses constant-time comparison to prevent timing-based key extraction.
6. **Error Sanitization** — Internal error details (parser state, file paths) are not exposed in HTTP responses.
7. **HTTPS** — Consider adding TLS for production deployments (requires certificate management).
8. **Provisioning Access** — The WiFi provisioning page (`/api/wifi` POST) is accessible without authentication on the SoftAP network by design.

## Troubleshooting

### Device won't connect to WiFi
- Check credentials in provisioning page
- Ensure WiFi network uses 2.4GHz band (ESP32 limitation)
- Try clearing credentials and re-provisioning

### OTA fails
- Verify OTA_KEY is correct
- Ensure filesystem image size <= 1MB
- Check device has enough free memory

### LittleFS mount fails
- May need to flash initial filesystem: `just flash-fs`
- Partition may be corrupted — erase and reflash: `just erase-fs` then `just create-fs static && just flash-fs`
