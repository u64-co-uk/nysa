/// Returns the MIME content type for a file based on its extension.
///
/// Supports common web file types (HTML, CSS, JS, JSON, images, fonts).
/// Returns `"application/octet-stream"` for unrecognized extensions.
pub fn get_content_type(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html"
    } else if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".woff") {
        "font/woff"
    } else if path.ends_with(".ttf") {
        "font/ttf"
    } else {
        "application/octet-stream"
    }
}

/// Validates and sanitizes a URI path for filesystem access.
///
/// Returns the corresponding filesystem path (prefixed with `/www`),
/// or `None` if the path contains traversal sequences (`..`),
/// null bytes, or percent-encoded traversal patterns.
pub fn sanitize_path(uri: &str) -> Option<String> {
    if uri.contains("..") || uri.contains('\0') {
        return None;
    }

    // Check for percent-encoded traversal (%2e = '.')
    let lower = uri.to_lowercase();
    if lower.contains("%2e%2e") || lower.contains("%2e.") || lower.contains(".%2e") {
        return None;
    }

    Some(format!("/www{}", uri))
}

/// Validates WiFi credentials against ESP32 constraints.
///
/// - SSID: 1-32 bytes (required, non-empty)
/// - Password: empty (open network) or 8-63 bytes (WPA2)
pub fn validate_wifi_credentials(ssid: &str, password: &str) -> Result<(), &'static str> {
    if ssid.is_empty() {
        return Err("SSID cannot be empty");
    }
    if ssid.len() > 32 {
        return Err("SSID must be 32 characters or fewer");
    }
    if !password.is_empty() && (password.len() < 8 || password.len() > 63) {
        return Err("Password must be 8-63 characters");
    }
    Ok(())
}

/// Performs constant-time string comparison to prevent timing attacks.
///
/// The comparison time does not vary based on which characters match,
/// preventing an attacker from extracting the key byte-by-byte.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    if a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a_bytes.iter().zip(b_bytes.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Strips internal error details before sending to HTTP clients.
///
/// Replaces serde parse errors, file paths, and other internal
/// information with generic user-facing messages.
pub fn sanitize_error_message(raw: &str) -> String {
    if raw.contains("missing field")
        || raw.contains("expected")
        || raw.contains("invalid type")
    {
        return "Invalid request format".to_string();
    }
    "Request processing failed".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- get_content_type ---

    #[test]
    fn content_type_html() {
        assert_eq!(get_content_type("index.html"), "text/html");
    }

    #[test]
    fn content_type_css() {
        assert_eq!(get_content_type("style.css"), "text/css");
    }

    #[test]
    fn content_type_js() {
        assert_eq!(get_content_type("app.js"), "application/javascript");
    }

    #[test]
    fn content_type_json() {
        assert_eq!(get_content_type("data.json"), "application/json");
    }

    #[test]
    fn content_type_png() {
        assert_eq!(get_content_type("image.png"), "image/png");
    }

    #[test]
    fn content_type_jpg() {
        assert_eq!(get_content_type("photo.jpg"), "image/jpeg");
    }

    #[test]
    fn content_type_jpeg() {
        assert_eq!(get_content_type("photo.jpeg"), "image/jpeg");
    }

    #[test]
    fn content_type_svg() {
        assert_eq!(get_content_type("icon.svg"), "image/svg+xml");
    }

    #[test]
    fn content_type_ico() {
        assert_eq!(get_content_type("favicon.ico"), "image/x-icon");
    }

    #[test]
    fn content_type_woff2() {
        assert_eq!(get_content_type("font.woff2"), "font/woff2");
    }

    #[test]
    fn content_type_woff() {
        assert_eq!(get_content_type("font.woff"), "font/woff");
    }

    #[test]
    fn content_type_ttf() {
        assert_eq!(get_content_type("font.ttf"), "font/ttf");
    }

    #[test]
    fn content_type_unknown() {
        assert_eq!(get_content_type("file.xyz"), "application/octet-stream");
    }

    #[test]
    fn content_type_no_extension() {
        assert_eq!(get_content_type("README"), "application/octet-stream");
    }

    #[test]
    fn content_type_nested_path() {
        assert_eq!(get_content_type("/www/assets/style.css"), "text/css");
    }

    // --- sanitize_path ---

    #[test]
    fn path_normal() {
        assert_eq!(
            sanitize_path("/index.html"),
            Some("/www/index.html".to_string())
        );
    }

    #[test]
    fn path_nested() {
        assert_eq!(
            sanitize_path("/assets/css/style.css"),
            Some("/www/assets/css/style.css".to_string())
        );
    }

    #[test]
    fn path_rejects_dot_dot() {
        assert_eq!(sanitize_path("/../etc/passwd"), None);
    }

    #[test]
    fn path_rejects_mid_traversal() {
        assert_eq!(sanitize_path("/assets/../../etc/passwd"), None);
    }

    #[test]
    fn path_rejects_encoded_traversal() {
        assert_eq!(sanitize_path("/%2e%2e/etc/passwd"), None);
    }

    #[test]
    fn path_rejects_mixed_encoded_traversal() {
        assert_eq!(sanitize_path("/%2e./etc/passwd"), None);
    }

    #[test]
    fn path_rejects_null_byte() {
        assert_eq!(sanitize_path("/index.html\0.jpg"), None);
    }

    #[test]
    fn path_root() {
        assert_eq!(sanitize_path("/"), Some("/www/".to_string()));
    }

    // --- validate_wifi_credentials ---

    #[test]
    fn wifi_valid_credentials() {
        assert!(validate_wifi_credentials("MyNetwork", "password123").is_ok());
    }

    #[test]
    fn wifi_empty_ssid_rejected() {
        assert!(validate_wifi_credentials("", "password123").is_err());
    }

    #[test]
    fn wifi_ssid_too_long() {
        let long_ssid = "a".repeat(33);
        assert!(validate_wifi_credentials(&long_ssid, "password123").is_err());
    }

    #[test]
    fn wifi_ssid_max_length_ok() {
        let ssid = "a".repeat(32);
        assert!(validate_wifi_credentials(&ssid, "password123").is_ok());
    }

    #[test]
    fn wifi_password_too_short() {
        assert!(validate_wifi_credentials("MyNet", "short").is_err());
    }

    #[test]
    fn wifi_password_too_long() {
        let long_pw = "a".repeat(64);
        assert!(validate_wifi_credentials("MyNet", &long_pw).is_err());
    }

    #[test]
    fn wifi_open_network_empty_password_ok() {
        assert!(validate_wifi_credentials("OpenNet", "").is_ok());
    }

    #[test]
    fn wifi_password_min_length() {
        assert!(validate_wifi_credentials("MyNet", "12345678").is_ok());
    }

    #[test]
    fn wifi_password_max_length() {
        let pw = "a".repeat(63);
        assert!(validate_wifi_credentials("MyNet", &pw).is_ok());
    }

    // --- constant_time_eq ---

    #[test]
    fn constant_time_eq_identical() {
        assert!(constant_time_eq("secret-key", "secret-key"));
    }

    #[test]
    fn constant_time_eq_different() {
        assert!(!constant_time_eq("secret-key", "wrong-key!"));
    }

    #[test]
    fn constant_time_eq_different_lengths() {
        assert!(!constant_time_eq("short", "longer-string"));
    }

    #[test]
    fn constant_time_eq_empty() {
        assert!(constant_time_eq("", ""));
    }

    #[test]
    fn constant_time_eq_one_empty() {
        assert!(!constant_time_eq("notempty", ""));
    }

    // --- sanitize_error_message ---

    #[test]
    fn sanitize_error_hides_missing_field() {
        let msg = sanitize_error_message("missing field `password` at line 1 column 23");
        assert!(!msg.contains("password"));
        assert!(!msg.contains("line 1"));
    }

    #[test]
    fn sanitize_error_hides_expected() {
        let msg = sanitize_error_message("expected value at line 1 column 1");
        assert!(!msg.contains("line"));
    }

    #[test]
    fn sanitize_error_generic() {
        let msg = sanitize_error_message("some unknown internal error occurred");
        assert!(!msg.contains("internal"));
    }
}
