use anyhow::Result;
use esp_idf_svc::http::server::{
    Configuration as HttpConfig, EspHttpConnection, EspHttpServer, Request,
};
use esp_idf_svc::http::Method;
use esp_idf_svc::io::Write;
use log::{error, info, warn};
use std::sync::{Arc, Mutex};

use crate::ota::OtaHandler;
use crate::storage::Storage;
use crate::wifi::WifiManager;
use nysa_utils::{
    constant_time_eq, get_content_type, sanitize_error_message, sanitize_path,
    validate_wifi_credentials,
};

const OTA_KEY: &str = env!("OTA_KEY", "default-ota-key-change-me");

const EMBEDDED_INDEX: &str = include_str!("../static/index.html");
const EMBEDDED_CONNECTED: &str = include_str!("../static/connected.html");
const EMBEDDED_404: &str = include_str!("../static/404.html");

/// HTTP server providing WiFi provisioning, device status, and OTA update
/// endpoints.
///
/// # Routes
///
/// | Method | Path         | Auth | Description                     |
/// |--------|-------------|------|---------------------------------|
/// | GET    | `/`          | No   | Provisioning or status page     |
/// | POST   | `/api/wifi`  | No   | Submit WiFi credentials         |
/// | GET    | `/api/status`| Yes  | Device status (JSON)            |
/// | DELETE | `/api/wifi`  | Yes  | Clear WiFi credentials          |
/// | POST   | `/ota/fs`    | Yes  | Upload filesystem image         |
/// | GET    | `/status`    | No   | Status page (direct)            |
/// | GET    | `/*`         | No   | Static file serving             |
pub struct WebServer {
    storage: Arc<Storage>,
    wifi_manager: Arc<Mutex<WifiManager>>,
}

impl WebServer {
    /// Creates a new [`WebServer`] with shared access to storage and WiFi.
    pub fn new(storage: Arc<Storage>, wifi_manager: Arc<Mutex<WifiManager>>) -> Result<Self> {
        Ok(Self {
            storage,
            wifi_manager,
        })
    }

    /// Starts the HTTP server and registers all route handlers.
    ///
    /// The server listens on port 80. Routes requiring authentication
    /// check the `X-OTA-Key` header against the compile-time OTA key.
    ///
    /// # Errors
    /// Returns an error if the HTTP server fails to bind or start.
    pub fn start(&self) -> Result<EspHttpServer<'_>> {
        if OTA_KEY == "change-me-in-production" {
            warn!("OTA_KEY is set to the default value! Change it for production use.");
        }

        let storage = self.storage.clone();
        let wifi_manager = self.wifi_manager.clone();

        let mut server = EspHttpServer::new(&HttpConfig {
            max_uri_handlers: 20,
            uri_match_wildcard: true,
            ..Default::default()
        })?;

        // Root route: serve setup page if not provisioned, status page if provisioned
        let storage_root = storage.clone();
        server.fn_handler("/", Method::Get, move |req| {
            let provisioned = storage_root.get_wifi_config().ok().flatten().is_some();
            if provisioned {
                // Prefer LittleFS index.html (custom app), fall back to embedded status page
                if std::path::Path::new("/www/index.html").exists() {
                    serve_file(req, "/www/index.html")
                } else {
                    serve_file(req, "/www/connected.html")
                }
            } else {
                serve_file(req, "/www/index.html")
            }
        })?;

        // WiFi provisioning endpoint
        let storage_clone = storage.clone();
        server.fn_handler(
            "/api/wifi",
            Method::Post,
            move |mut req| -> anyhow::Result<()> {
                let mut body = [0u8; 512];
                let len = match req.read(&mut body) {
                    Ok(n) => n,
                    Err(e) => {
                        error!("Failed to read request body: {}", e);
                        let mut response = req.into_status_response(400)?;
                        response.write_all(b"Failed to read request")?;
                        return Ok(());
                    }
                };

                let body_str = match std::str::from_utf8(&body[..len]) {
                    Ok(s) => s,
                    Err(_) => {
                        let mut response = req.into_status_response(400)?;
                        response.write_all(b"Invalid UTF-8")?;
                        return Ok(());
                    }
                };

                info!("WiFi provisioning request received");

                match serde_json::from_str::<crate::storage::WifiConfig>(body_str) {
                    Ok(config) => {
                        let (ssid, password) = (config.ssid, config.password);

                        // Validate credentials before saving
                        if let Err(msg) = validate_wifi_credentials(&ssid, &password) {
                            let mut response = req.into_status_response(400)?;
                            response.write_all(msg.as_bytes())?;
                            return Ok(());
                        }

                        info!("WiFi credentials received");

                        // Save credentials first
                        if let Err(e) = storage_clone.save_wifi_config(&ssid, &password) {
                            error!("Failed to save WiFi config: {}", e);
                            let mut response = req.into_status_response(500)?;
                            response.write_all(b"Failed to save configuration")?;
                            return Ok(());
                        }

                        // Send success response
                        let mut response = req.into_ok_response()?;
                        response.write_all(b"WiFi credentials saved. Rebooting...")?;

                        // Reboot after a short delay to allow response to be sent
                        info!("Rebooting to connect to new WiFi network...");
                        std::thread::spawn(|| {
                            std::thread::sleep(std::time::Duration::from_millis(500));
                            unsafe {
                                esp_idf_svc::hal::sys::esp_restart();
                            }
                        });

                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to parse WiFi JSON: {}", e);
                        let mut response = req.into_status_response(400)?;
                        response.write_all(sanitize_error_message(&e.to_string()).as_bytes())?;
                        Ok(())
                    }
                }
            },
        )?;

        // API Status endpoint (requires auth)
        let storage_clone2 = storage.clone();
        server.fn_handler(
            "/api/status",
            Method::Get,
            move |req| -> anyhow::Result<()> {
                if !check_auth(&req) {
                    let mut response = req.into_status_response(401)?;
                    response.write_all(b"Unauthorized")?;
                    return Ok(());
                }

                let wifi_info = match wifi_manager.lock() {
                    Ok(mgr) => {
                        let connected = mgr.is_connected();
                        let ip = mgr.get_ip().unwrap_or_else(|| "Unknown".to_string());
                        (connected, ip)
                    }
                    Err(_) => (false, "Unknown".to_string()),
                };

                let config = storage_clone2.get_wifi_config().unwrap_or(None);
                let ssid = config
                    .map(|c| c.ssid)
                    .unwrap_or_else(|| "Not Configured".to_string());

                // Get uptime
                let uptime = unsafe { esp_idf_svc::hal::sys::esp_timer_get_time() } / 1_000_000; // Convert to seconds

                let json = format!(
                    "{{\"connected\":{},\"ssid\":\"{}\",\"ip\":\"{}\",\"uptime\":{}}}",
                    wifi_info.0, ssid, wifi_info.1, uptime
                );

                let mut response =
                    req.into_response(200, Some("OK"), &[("Content-Type", "application/json")])?;
                response.write_all(json.as_bytes())?;
                Ok(())
            },
        )?;

        // WiFi forget endpoint (requires auth)
        let storage_clone3 = storage.clone();
        server.fn_handler(
            "/api/wifi",
            Method::Delete,
            move |req| -> anyhow::Result<()> {
                if !check_auth(&req) {
                    let mut response = req.into_status_response(401)?;
                    response.write_all(b"Unauthorized")?;
                    return Ok(());
                }

                match storage_clone3.clear_wifi_config() {
                    Ok(_) => {
                        let mut response = req.into_ok_response()?;
                        response.write_all(b"WiFi credentials cleared")?;

                        // Reboot after delay
                        std::thread::spawn(|| {
                            std::thread::sleep(std::time::Duration::from_millis(500));
                            unsafe {
                                esp_idf_svc::hal::sys::esp_restart();
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to clear WiFi config: {}", e);
                        let mut response = req.into_status_response(500)?;
                        response.write_all(b"Failed to clear WiFi configuration")?;
                    }
                }
                Ok(())
            },
        )?;

        // OTA endpoint for filesystem updates (requires auth, streams directly to flash)
        server.fn_handler("/ota/fs", Method::Post, |mut req| -> anyhow::Result<()> {
            if !check_auth(&req) {
                error!("Unauthorized OTA attempt");
                let mut response = req.into_status_response(401)?;
                response.write_all(b"Unauthorized")?;
                return Ok(());
            }

            // Stream directly to flash partition (no buffering in RAM)
            let ota = OtaHandler::new();
            match ota.handle_fs_ota_stream(&mut req) {
                Ok(_) => {
                    let mut response = req.into_ok_response()?;
                    response
                        .write_all(b"Filesystem updated successfully. Reboot to apply changes.")?;

                    // Schedule reboot
                    std::thread::spawn(|| {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        unsafe {
                            esp_idf_svc::hal::sys::esp_restart();
                        }
                    });
                }
                Err(e) => {
                    error!("OTA failed: {}", e);
                    let mut response = req.into_status_response(500)?;
                    response.write_all(b"OTA update failed")?;
                }
            }
            Ok(())
        })?;

        // Status/upload page (always available, even after OTA)
        server.fn_handler("/status", Method::Get, |req| {
            serve_file(req, "/www/connected.html")
        })?;

        // Wildcard file handler (must be registered LAST — catches all remaining GET requests)
        server.fn_handler("/*", Method::Get, move |req| -> anyhow::Result<()> {
            let uri = req.uri();
            match sanitize_path(uri) {
                Some(path) => serve_file(req, &path),
                None => {
                    let mut response = req.into_status_response(400)?;
                    response.write_all(b"Invalid path")?;
                    Ok(())
                }
            }
        })?;

        info!("HTTP server started on port 80");
        Ok(server)
    }
}

fn serve_file(req: Request<&mut EspHttpConnection>, path: &str) -> Result<(), anyhow::Error> {
    // Try LittleFS first, then fall back to embedded defaults
    if let Ok(content) = std::fs::read(path) {
        let content_type = get_content_type(path);
        let mut response = req.into_response(200, Some("OK"), &[("Content-Type", content_type)])?;
        response.write_all(&content)?;
    } else if let Some((content, content_type)) = get_embedded_file(path) {
        let mut response = req.into_response(200, Some("OK"), &[("Content-Type", content_type)])?;
        response.write_all(content.as_bytes())?;
    } else {
        // 404: try LittleFS 404.html, then embedded, then plain text
        let mut response = req.into_status_response(404)?;
        if let Ok(content) = std::fs::read("/www/404.html") {
            response.write_all(&content)?;
        } else {
            response.write_all(EMBEDDED_404.as_bytes())?;
        }
    }
    Ok(())
}

fn get_embedded_file(path: &str) -> Option<(&'static str, &'static str)> {
    match path {
        "/www/index.html" => Some((EMBEDDED_INDEX, "text/html")),
        "/www/connected.html" => Some((EMBEDDED_CONNECTED, "text/html")),
        "/www/404.html" => Some((EMBEDDED_404, "text/html")),
        _ => None,
    }
}

fn check_auth(req: &Request<&mut EspHttpConnection>) -> bool {
    req.header("X-OTA-Key")
        .map(|v| constant_time_eq(v, OTA_KEY))
        .unwrap_or(false)
}
