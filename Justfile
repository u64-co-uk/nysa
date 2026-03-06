# Nysa ESP32 HTTP Server - Build Automation

# Set default port (override with ESP_PORT environment variable)
set windows-shell := ["cmd", "/c"]

# Default ESP port
ESP_PORT := env_var_or_default("ESP_PORT", "COM3")

# Show available commands
[private]
default:
    @echo Nysa ESP32 HTTP Server - Build System
    @echo.
    @echo Getting started:
    @echo   just setup          - Install required tools (one-time)
    @echo.
    @echo Build:
    @echo   just build          - Build firmware (release)
    @echo   just build-key KEY  - Build with custom OTA key
    @echo   just build-fast     - Quick debug build (no checks)
    @echo.
    @echo Flash ^& monitor:
    @echo   just flash          - Build, flash, and monitor
    @echo   just flash-fs       - Flash filesystem image only
    @echo   just monitor        - Monitor serial output
    @echo   just all            - Erase NVS + LittleFS, build, flash, and monitor
    @echo.
    @echo Filesystem:
    @echo   just create-fs DIR  - Create LittleFS image from directory
    @echo   just erase-fs       - Erase LittleFS partition
    @echo   just erase-nvs      - Erase NVS (clear saved WiFi)
    @echo.
    @echo Code quality:
    @echo   just fmt            - Format code
    @echo   just check          - Check code without building
    @echo   just clippy         - Run clippy linter
    @echo   just test-unit      - Run unit tests (no ESP32 needed)
    @echo   just test           - Full check: fmt + check + clippy + unit tests
    @echo.
    @echo Other:
    @echo   just clean          - Clean build artifacts
    @echo.
    @echo Environment variables:
    @echo   ESP_PORT            - Serial port (default: COM3)
    @echo   OTA_KEY             - OTA security key
    @echo.
    @echo NOTE: The esp toolchain is auto-selected via rust-toolchain.toml.
    @echo If the build can't find ESP-IDF tools, run:
    @echo   %USERPROFILE%\.espup\esp\export-esp.bat

# ESP environment check
[private]
check-esp:
    @echo Checking ESP environment...
    @rustc --print target-list | findstr xtensa >nul && echo ✓ ESP toolchain found || (echo ✗ ESP toolchain not found! Run: %USERPROFILE%\.espup\esp\export-esp.bat && exit /b 1)

# Install required tools
setup:
    @echo Installing required tools...
    cargo install espup
    espup install
    cargo install espflash
    cargo install cargo-espflash
    cargo install ldproxy
    pip install littlefs-tools
    @echo.
    @echo Setup complete!

# Build firmware (with ESP check)
build: check-esp
    cargo build --release

# Build firmware with custom OTA key
build-key KEY: check-esp
    set "OTA_KEY={{KEY}}" && cargo build --release

# Create LittleFS image from a directory (1MB partition, 4096 block size, 256 blocks)
create-fs DIR:
    @echo Creating LittleFS image from {{DIR}}...
    python3 -m littlefs create --block-size 4096 --block-count 256 {{DIR}} target\littlefs.bin
    @echo.
    @echo Created: target\littlefs.bin
    @echo To flash via serial: just flash-fs
    @echo To upload via OTA:
    @echo   curl -X POST http://DEVICE_IP/ota/fs -H "X-OTA-Key: your-key" --data-binary @target\littlefs.bin

# Build, flash, and monitor
flash: build
    cargo espflash flash --release --port {{ESP_PORT}} --partition-table partitions.csv -M

# Flash filesystem image only
flash-fs:
    @echo Flashing filesystem to {{ESP_PORT}}...
    @if exist target\littlefs.bin (espflash write-bin --partition-table partitions.csv --port {{ESP_PORT}} 0x190000 target\littlefs.bin) else (echo Error: target\littlefs.bin not found! Run 'just create-fs DIR' first. && exit /b 1)

# Monitor serial output
monitor:
    espflash monitor --port {{ESP_PORT}}

# Erase NVS partition (clears saved WiFi credentials)
erase-nvs:
    @echo Erasing NVS partition on {{ESP_PORT}}...
    espflash erase-region --port {{ESP_PORT}} 0x9000 0x6000

# Erase LittleFS partition (clears uploaded files, reverts to embedded defaults)
erase-fs:
    @echo Erasing LittleFS partition on {{ESP_PORT}}...
    espflash erase-region --port {{ESP_PORT}} 0x190000 0x100000

# Erase NVS + LittleFS, build, flash, and monitor
all: erase-nvs erase-fs flash

# Clean build artifacts
clean:
    cargo clean
    @if exist target\littlefs.bin del /Q target\littlefs.bin 2>nul
    @echo Clean complete!

# Format code
fmt: check-esp
    cargo fmt

# Check code without building
check: check-esp
    cargo check

# Run clippy for linting
clippy: check-esp
    cargo clippy --release -- -D warnings

# Run unit tests (on host, no ESP32 needed)
test-unit:
    cargo +stable test --manifest-path nysa-utils\Cargo.toml --target x86_64-pc-windows-msvc

# Full check: format, check, clippy, and unit tests
test: fmt check clippy test-unit
    @echo All checks passed!

# Quick build without checks (for testing)
build-fast: check-esp
    cargo build
