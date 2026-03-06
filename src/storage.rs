use anyhow::Result;
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};
use log::info;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

const NVS_NAMESPACE: &str = "nysa_cfg";
const KEY_WIFI_SSID: &str = "wifi_ssid";
const KEY_WIFI_PASS: &str = "wifi_pass";

/// Persistent WiFi network credentials.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WifiConfig {
    pub ssid: String,
    pub password: String,
}

/// NVS (Non-Volatile Storage) wrapper for persisting device configuration
/// across reboots.
///
/// Currently stores WiFi credentials. All operations are thread-safe
/// via an internal mutex.
pub struct Storage {
    nvs: Mutex<EspNvs<NvsDefault>>,
}

impl Storage {
    /// Creates a new [`Storage`] instance backed by the ESP32's NVS partition.
    ///
    /// # Errors
    /// Returns an error if the NVS namespace cannot be opened.
    pub fn new(partition: EspDefaultNvsPartition) -> Result<Self> {
        let nvs = EspNvs::new(partition, NVS_NAMESPACE, true)?;
        Ok(Self {
            nvs: Mutex::new(nvs),
        })
    }

    /// Saves WiFi credentials to persistent storage.
    ///
    /// Validates SSID and password length before writing. This is a
    /// defense-in-depth check; the web server also validates input.
    ///
    /// # Errors
    /// Returns an error if the SSID or password fails length validation,
    /// or if the NVS write fails.
    pub fn save_wifi_config(&self, ssid: &str, password: &str) -> Result<()> {
        if ssid.is_empty() || ssid.len() > 32 {
            return Err(anyhow::anyhow!("Invalid SSID length"));
        }
        if !password.is_empty() && (password.len() < 8 || password.len() > 63) {
            return Err(anyhow::anyhow!("Invalid password length"));
        }

        let nvs = self.nvs.lock().unwrap();
        nvs.set_str(KEY_WIFI_SSID, ssid)?;
        nvs.set_str(KEY_WIFI_PASS, password)?;
        info!("WiFi configuration saved to NVS");
        Ok(())
    }

    /// Retrieves stored WiFi credentials, if any.
    ///
    /// Returns `Ok(None)` if no credentials have been saved or if the
    /// stored SSID is empty.
    pub fn get_wifi_config(&self) -> Result<Option<WifiConfig>> {
        let nvs = self.nvs.lock().unwrap();
        let mut ssid_buf = [0u8; 128];
        let mut pass_buf = [0u8; 128];

        let ssid = match nvs.get_str(KEY_WIFI_SSID, &mut ssid_buf)? {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => return Ok(None),
        };

        let password = match nvs.get_str(KEY_WIFI_PASS, &mut pass_buf)? {
            Some(p) => p.to_string(),
            _ => return Ok(None),
        };

        Ok(Some(WifiConfig { ssid, password }))
    }

    /// Clears stored WiFi credentials, causing the device to enter
    /// provisioning mode on the next reboot.
    pub fn clear_wifi_config(&self) -> Result<()> {
        let nvs = self.nvs.lock().unwrap();
        nvs.set_str(KEY_WIFI_SSID, "")?;
        nvs.set_str(KEY_WIFI_PASS, "")?;
        info!("WiFi configuration cleared from NVS");
        Ok(())
    }
}
