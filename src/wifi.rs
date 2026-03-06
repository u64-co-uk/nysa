use anyhow::Result;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::modem::Modem;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use log::info;

/// SoftAP network name shown during provisioning.
const AP_SSID: &str = "Nysa-Setup";
/// SoftAP password for provisioning mode.
const AP_PASSWORD: &str = "nysa-setup-123";
/// SoftAP gateway IP address.
const AP_IP: &str = "192.168.71.1";

/// Manages WiFi connectivity for the ESP32, supporting both SoftAP
/// (access point) mode for initial provisioning and STA (station) mode
/// for connecting to an existing network.
pub struct WifiManager {
    wifi: BlockingWifi<EspWifi<'static>>,
}

impl WifiManager {
    /// Creates a new [`WifiManager`] wrapping the ESP32 modem peripheral.
    ///
    /// # Errors
    /// Returns an error if the WiFi driver fails to initialize.
    pub fn new(
        modem: Modem,
        sysloop: EspSystemEventLoop,
        nvs: EspDefaultNvsPartition,
    ) -> Result<Self> {
        let wifi = BlockingWifi::wrap(
            EspWifi::new(modem, sysloop.clone(), Some(nvs.clone()))?,
            sysloop,
        )?;

        Ok(Self { wifi })
    }

    /// Starts SoftAP mode for WiFi provisioning.
    ///
    /// Creates a wireless access point with SSID "Nysa-Setup" that users
    /// can connect to for initial device configuration via the web UI.
    ///
    /// # Errors
    /// Returns an error if the access point fails to start.
    pub fn start_provisioning_mode(&mut self) -> Result<()> {
        info!("Starting SoftAP for provisioning...");

        let ap_config = Configuration::AccessPoint(esp_idf_svc::wifi::AccessPointConfiguration {
            ssid: AP_SSID.try_into().unwrap(),
            password: AP_PASSWORD.try_into().unwrap(),
            channel: 1,
            ..Default::default()
        });

        self.wifi.set_configuration(&ap_config)?;
        self.wifi.start()?;

        info!("SoftAP started: {}", AP_SSID);
        info!(
            "Connect to this network and visit http://{}/ to provision",
            AP_IP
        );

        Ok(())
    }

    /// Connects to a WiFi network in station (client) mode.
    ///
    /// Blocks until the connection is established and an IP address is obtained.
    ///
    /// # Errors
    /// Returns an error if the SSID/password exceeds ESP32 limits,
    /// or if the connection fails.
    pub fn connect_sta(&mut self, ssid: &str, password: &str) -> Result<()> {
        info!("Connecting to WiFi network...");

        let sta_config = Configuration::Client(ClientConfiguration {
            ssid: ssid
                .try_into()
                .map_err(|_| anyhow::anyhow!("SSID too long for ESP32 (max 32 bytes)"))?,
            password: password
                .try_into()
                .map_err(|_| anyhow::anyhow!("Password too long for ESP32 (max 64 bytes)"))?,
            ..Default::default()
        });

        self.wifi.set_configuration(&sta_config)?;
        self.wifi.start()?;
        self.wifi.connect()?;
        self.wifi.wait_netif_up()?;

        let ip_info = self.wifi.wifi().sta_netif().get_ip_info()?;
        info!("Connected! IP: {}", ip_info.ip);

        Ok(())
    }

    /// Returns the device's current IP address, or `None` if not connected.
    pub fn get_ip(&self) -> Option<String> {
        self.wifi
            .wifi()
            .sta_netif()
            .get_ip_info()
            .ok()
            .map(|info| info.ip.to_string())
    }

    /// Returns `true` if the device is currently connected to a WiFi network.
    pub fn is_connected(&self) -> bool {
        self.wifi.is_connected().unwrap_or(false)
    }
}
