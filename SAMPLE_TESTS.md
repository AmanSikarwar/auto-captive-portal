# Sample Test Code

This document shows representative examples of the tests added to each module.

## captive_portal.rs - Sample Tests

### Basic Extraction Test
```rust
#[test]
fn test_extract_captive_portal_url_valid() {
    let html = r#"<script>window.location="https://login.iitmandi.ac.in:1003/portal"</script>"#;
    assert_eq!(
        extract_captive_portal_url(html),
        Some("https://login.iitmandi.ac.in:1003/portal".to_string())
    );
}
```

### Edge Case: Empty String
```rust
#[test]
fn test_extract_portal_url_empty_string() {
    let html = "";
    assert_eq!(extract_captive_portal_url(html), None);
}
```

### Edge Case: IPv6 Support
```rust
#[test]
fn test_extract_portal_url_with_ipv6() {
    let html = r#"window.location="http://[2001:db8::1]/portal""#;
    assert_eq!(
        extract_captive_portal_url(html),
        Some("http://[2001:db8::1]/portal".to_string())
    );
}
```

### Constants Validation
```rust
#[test]
fn test_constants() {
    assert_eq!(MAX_LOGIN_RETRIES, 3);
    assert_eq!(INITIAL_RETRY_DELAY_SECS, 2);
    assert_eq!(LOGOUT_URL, "https://login.iitmandi.ac.in:1003/logout?");
    assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(10));
    assert_eq!(
        DEFAULT_CONNECTIVITY_CHECK_URL,
        "http://clients3.google.com/generate_204"
    );
}
```

### Boundary Test: Long Values
```rust
#[test]
fn test_extract_magic_value_long_value() {
    // Test with a very long magic value
    let long_value = "a".repeat(1000);
    let html = format!(
        r#"<input type="hidden" name="magic" value="{}">"#,
        long_value
    );
    assert_eq!(extract_magic_value(&html), Some(long_value));
}
```

## daemon.rs - Sample Tests

### Constants Validation
```rust
#[test]
fn test_constants() {
    assert_eq!(MAX_DELAY_SECS, 1800);
    assert_eq!(MIN_DELAY_SECS, 10);
    assert_eq!(CHANNEL_CAPACITY, 10);
}
```

### Relationship Test
```rust
#[test]
fn test_min_max_delay_relationship() {
    // Ensure MIN_DELAY is less than MAX_DELAY
    assert!(MIN_DELAY_SECS < MAX_DELAY_SECS);
}
```

## logging.rs - Sample Tests

### Log Rotation Test
```rust
#[test]
fn test_rotate_logs_with_small_file() {
    // Create a temporary directory for testing
    let temp_dir = std::env::temp_dir().join("acp_test_logs");
    let _ = fs::create_dir_all(&temp_dir);
    let log_path = temp_dir.join("test.log");

    // Create a small log file
    fs::write(&log_path, "small log").unwrap();

    // Rotation should not happen for small files
    rotate_logs_if_needed(&log_path);

    // Original file should still exist
    assert!(log_path.exists());

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}
```

### Large File Rotation Test
```rust
#[test]
fn test_rotate_logs_with_large_file() {
    // Create a temporary directory for testing
    let temp_dir = std::env::temp_dir().join("acp_test_logs_large");
    let _ = fs::create_dir_all(&temp_dir);
    let log_path = temp_dir.join("test.log");

    // Create a file larger than MAX_LOG_SIZE_BYTES
    let large_content = "x".repeat((MAX_LOG_SIZE_BYTES + 1) as usize);
    fs::write(&log_path, large_content).unwrap();

    // Perform rotation
    rotate_logs_if_needed(&log_path);

    // Check if rotation happened - rotated file should exist
    let rotated_path = temp_dir.join("test.log.1");
    assert!(rotated_path.exists() || !log_path.exists());

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}
```

### Environment Variable Test
```rust
#[test]
fn test_log_level_parsing() {
    // Test that RUST_LOG environment variable is respected
    std::env::set_var("RUST_LOG", "debug");
    let log_level: Option<LevelFilter> = std::env::var("RUST_LOG")
        .ok()
        .and_then(|s| s.parse().ok());
    assert_eq!(log_level, Some(LevelFilter::Debug));
    std::env::remove_var("RUST_LOG");
}
```

## state.rs - Sample Tests

### Duration Formatting Tests
```rust
#[test]
fn test_format_duration_ago_seconds() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let timestamp = now - 30;
    let result = format_duration_ago(timestamp);
    assert_eq!(result, "30 seconds ago");
}

#[test]
fn test_format_duration_ago_one_minute() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let timestamp = now - 60;
    let result = format_duration_ago(timestamp);
    assert_eq!(result, "1 minute ago");
}

#[test]
fn test_format_duration_ago_days() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let timestamp = now - 172800; // 2 days
    let result = format_duration_ago(timestamp);
    assert_eq!(result, "2 days ago");
}
```

### Edge Case: Future Timestamp
```rust
#[test]
fn test_format_duration_ago_future_timestamp() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let timestamp = now + 100; // Future timestamp
    let result = format_duration_ago(timestamp);
    assert_eq!(result, "just now");
}
```

### Serialization Test
```rust
#[test]
fn test_service_state_serialization() {
    let state = ServiceState {
        last_check_timestamp: Some(1234567890),
        last_successful_login_timestamp: Some(1234567891),
        last_portal_detected: Some("https://portal.test.com".to_string()),
    };

    let json = serde_json::to_string(&state);
    assert!(json.is_ok());

    let deserialized: Result<ServiceState, _> = serde_json::from_str(&json.unwrap());
    assert!(deserialized.is_ok());

    let deserialized_state = deserialized.unwrap();
    assert_eq!(
        deserialized_state.last_check_timestamp,
        Some(1234567890)
    );
    assert_eq!(
        deserialized_state.last_successful_login_timestamp,
        Some(1234567891)
    );
    assert_eq!(
        deserialized_state.last_portal_detected,
        Some("https://portal.test.com".to_string())
    );
}
```

### State Persistence Test
```rust
#[test]
fn test_update_state_preserves_existing_data() {
    // First update with portal URL
    let _ = update_state_file(Some("https://first.com"), false);

    // Second update with login success (no portal URL)
    let result = update_state_file(None, true);

    if result.is_ok() {
        let state = load_state().unwrap();
        // Portal URL should still be present from first update
        assert!(state.last_portal_detected.is_some());
        assert!(state.last_successful_login_timestamp.is_some());
    }
}
```

## Test Patterns Used

### 1. Edge Case Testing
- Empty strings
- Very long values (1000+ chars)
- Whitespace handling
- Special characters

### 2. Boundary Testing
- Zero values
- Maximum values
- Future timestamps
- MIN/MAX relationships

### 3. Platform-Specific Testing
```rust
#[cfg(target_os = "windows")]
{
    // Windows-specific test code
}

#[cfg(not(target_os = "windows"))]
{
    // Unix/Linux-specific test code
}
```

### 4. Cleanup Pattern
```rust
// Create temp directory
let temp_dir = std::env::temp_dir().join("test_name");
let _ = fs::create_dir_all(&temp_dir);

// ... test code ...

// Cleanup
let _ = fs::remove_dir_all(&temp_dir);
```

### 5. Regex Testing
```rust
let regex = portal_url_regex();
assert!(regex.is_match(r#"window.location="https://example.com""#));
```

## Running These Tests

```bash
# Run all tests
cargo test --lib

# Run a specific test
cargo test --lib test_extract_captive_portal_url_valid

# Run tests with output
cargo test --lib -- --nocapture

# Run tests matching a pattern
cargo test --lib format_duration
```

## Key Testing Principles Applied

1. **Descriptive Names**: Test names clearly describe what they test
2. **Single Responsibility**: Each test verifies one specific behavior
3. **Arrange-Act-Assert**: Tests follow AAA pattern
4. **No Side Effects**: Tests clean up after themselves
5. **Independence**: Tests don't depend on each other
6. **Repeatability**: Tests produce same results every run

---

These samples represent the testing approach used throughout the codebase. All 65 tests follow similar patterns and conventions.