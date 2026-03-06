//! Nysa — a lightweight HTTP server for ESP32-WROOM-32 with WiFi
//! provisioning and filesystem OTA updates.
//!
//! On first boot, the device starts a SoftAP named "Nysa-Setup" for WiFi
//! configuration. After provisioning, it connects to the configured network
//! and serves files from a LittleFS partition, falling back to compiled-in
//! defaults if the filesystem is unavailable.

use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::prelude::*;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use log::info;
use std::sync::Arc;

mod ota;
mod storage;
mod web_server;
mod wifi;

use storage::Storage;
use web_server::WebServer;
use wifi::WifiManager;

const LITTLEFS_MOUNT_POINT: &str = "/www";

fn main() -> anyhow::Result<()> {
    // Initialize ESP-IDF
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Nysa HTTP Server starting...");

    let peripherals = Peripherals::take()?;
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // Initialize storage (NVS)
    let storage = Arc::new(Storage::new(nvs.clone())?);

    // Mount LittleFS filesystem (non-fatal — embedded defaults will be used if unavailable)
    if let Err(e) = mount_littlefs() {
        log::warn!("LittleFS mount failed: {}. Using embedded files only.", e);
    }

    // Check if we have WiFi credentials
    let wifi_config = storage.get_wifi_config()?;

    let wifi_manager = Arc::new(std::sync::Mutex::new(WifiManager::new(
        peripherals.modem,
        sysloop.clone(),
        nvs.clone(),
    )?));

    let should_provision = wifi_config.is_none();

    if should_provision {
        info!("No WiFi credentials found. Starting provisioning mode...");
        wifi_manager.lock().unwrap().start_provisioning_mode()?;
    } else {
        info!("Connecting to saved WiFi network...");
        let config = wifi_config.unwrap();
        wifi_manager
            .lock()
            .unwrap()
            .connect_sta(&config.ssid, &config.password)?;
    }

    // Start web server with retry logic
    let server = WebServer::new(storage.clone(), wifi_manager.clone())?;
    let _server = {
        const MAX_RETRIES: u32 = 5;
        let mut last_err = None;
        let mut started = None;
        for attempt in 1..=MAX_RETRIES {
            match server.start() {
                Ok(s) => {
                    started = Some(s);
                    break;
                }
                Err(e) => {
                    log::warn!(
                        "HTTP server start failed (attempt {}/{}): {}",
                        attempt,
                        MAX_RETRIES,
                        e
                    );
                    last_err = Some(e);
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
            }
        }
        match started {
            Some(s) => s,
            None => {
                log::error!(
                    "HTTP server failed after {} retries: {}. Rebooting...",
                    MAX_RETRIES,
                    last_err.unwrap()
                );
                std::thread::sleep(std::time::Duration::from_millis(100));
                unsafe {
                    esp_idf_svc::hal::sys::esp_restart();
                }
            }
        }
    };

    info!("Nysa HTTP Server running!");

    // Main loop
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn mount_littlefs() -> anyhow::Result<()> {
    use esp_idf_svc::sys::*;
    info!("Mounting LittleFS filesystem...");

    let conf = esp_vfs_littlefs_conf_t {
        base_path: c"/www".as_ptr(),
        partition_label: c"storage".as_ptr(),
        _bitfield_1: esp_vfs_littlefs_conf_t::new_bitfield_1(
            1, // format_if_mount_failed
            0, // read_only
            0, // dont_mount
            0, // grow_on_mount
        ),
        ..Default::default()
    };
    esp!(unsafe { esp_vfs_littlefs_register(&conf) })?;

    info!("LittleFS mounted at {}", LITTLEFS_MOUNT_POINT);
    Ok(())
}
