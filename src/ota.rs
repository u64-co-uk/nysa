use anyhow::{anyhow, Result};
use esp_idf_svc::http::server::{EspHttpConnection, Request};
use log::info;

/// Handles over-the-air filesystem updates by streaming data directly
/// to the ESP32's storage partition.
pub struct OtaHandler;

impl OtaHandler {
    /// Creates a new [`OtaHandler`].
    pub fn new() -> Self {
        Self
    }

    /// Streams a filesystem OTA update directly from an HTTP request to the
    /// storage partition. Reads and writes in 4KB chunks to avoid buffering
    /// the entire image in memory.
    ///
    /// # Security
    /// The caller must verify authentication before invoking this method.
    /// The storage partition is erased before writing begins.
    ///
    /// # Errors
    /// Returns an error if the storage partition is not found, the image
    /// exceeds the partition size, or a flash write fails.
    pub fn handle_fs_ota_stream(&self, req: &mut Request<&mut EspHttpConnection>) -> Result<()> {
        use esp_idf_svc::sys::*;

        info!("Starting streaming filesystem OTA...");

        // Find the storage partition
        let partition = unsafe {
            esp_partition_find_first(
                esp_partition_type_t_ESP_PARTITION_TYPE_DATA,
                esp_partition_subtype_t_ESP_PARTITION_SUBTYPE_ANY,
                c"storage".as_ptr(),
            )
        };
        if partition.is_null() {
            return Err(anyhow!("Storage partition not found"));
        }

        let partition_size = unsafe { (*partition).size } as usize;
        info!("Storage partition size: {} bytes", partition_size);

        // Erase the entire partition before writing
        info!("Erasing storage partition...");
        esp!(unsafe { esp_partition_erase_range(partition, 0, partition_size) })?;

        // Stream from HTTP request directly to flash in 4KB chunks
        const CHUNK_SIZE: usize = 4096;
        let mut buffer = [0u8; CHUNK_SIZE];
        let mut total_written = 0usize;

        loop {
            let n = req
                .read(&mut buffer)
                .map_err(|e| anyhow!("Read error: {}", e))?;
            if n == 0 {
                break;
            }

            if total_written + n > partition_size {
                return Err(anyhow!(
                    "Image exceeds partition size ({} bytes)",
                    partition_size
                ));
            }

            esp!(unsafe {
                esp_partition_write(
                    partition,
                    total_written,
                    buffer[..n].as_ptr() as *const _,
                    n,
                )
            })?;

            total_written += n;
        }

        info!("Filesystem OTA completed: {} bytes written", total_written);
        Ok(())
    }
}
