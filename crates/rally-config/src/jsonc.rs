/// Strip JSONC comments (`//` line comments and `/* */` block comments)
/// from a string, returning valid JSON.
pub fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            // Line comment — skip to end of line
            i += 2;
            while i < len && bytes[i] != b'\n' {
                i += 1;
            }
        } else if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            // Block comment — skip to */
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            if i + 1 < len {
                i += 2; // skip */
            }
        } else if bytes[i] == b'"' {
            // String literal — copy verbatim, respecting escapes
            out.push('"');
            i += 1;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    out.push(bytes[i] as char);
                    out.push(bytes[i + 1] as char);
                    i += 2;
                } else if bytes[i] == b'"' {
                    out.push('"');
                    i += 1;
                    break;
                } else {
                    out.push(bytes[i] as char);
                    i += 1;
                }
            }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_line_comments() {
        let input = r#"{
  // This is a comment
  "key": "value" // inline comment
}"#;
        let result = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn strips_block_comments() {
        let input = r#"{
  /* block comment */
  "key": /* inline */ "value"
}"#;
        let result = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn preserves_strings_with_slashes() {
        let input = r#"{ "url": "https://example.com/path" }"#;
        let result = strip_jsonc_comments(input);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["url"], "https://example.com/path");
    }

    #[test]
    fn default_config_roundtrips() {
        let config = crate::RallyConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: crate::RallyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.daemon.log_level, "info");
        assert_eq!(parsed.mcp.http_port, 8377);
        assert_eq!(parsed.capture.ring_buffer_mb, 16);
    }

    #[test]
    fn partial_config_fills_defaults() {
        let input = r#"{ "daemon": { "log_level": "debug" } }"#;
        let config: crate::RallyConfig = serde_json::from_str(input).unwrap();
        assert_eq!(config.daemon.log_level, "debug");
        // Other fields should be defaults
        assert_eq!(config.mcp.http_port, 8377);
        assert_eq!(config.capture.poll_hz, 5);
        assert!(config.daemon.socket_path.is_none());
    }
}
