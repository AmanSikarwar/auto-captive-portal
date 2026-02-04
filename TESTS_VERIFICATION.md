# Test Verification Guide

This document provides instructions for verifying the comprehensive tests added to this project.

## Quick Start

```bash
# Run the test runner script
./run_tests.sh

# Or run cargo test directly
cargo test --lib
```

## Test Organization

All tests are organized as inline test modules using Rust's `#[cfg(test)]` attribute:

```
src/
├── captive_portal.rs  (36 tests total: 5 existing + 31 new)
├── daemon.rs          (4 new tests)
├── logging.rs         (11 new tests)
└── state.rs           (19 new tests)
```

## Verification Checklist

### 1. Compile Check
```bash
# Ensure all tests compile
cargo test --lib --no-run
```

Expected: All tests should compile without errors.

### 2. Run All Tests
```bash
# Run all library tests
cargo test --lib
```

Expected: All 65+ tests should pass.

### 3. Module-Specific Tests

#### Captive Portal Tests (36 tests)
```bash
cargo test --lib captive_portal
```

Tests cover:
- ✅ URL extraction from HTML (various formats)
- ✅ Magic value extraction
- ✅ Edge cases (empty, whitespace, special chars)
- ✅ IPv4 and IPv6 support
- ✅ Malformed input handling
- ✅ Constants validation

Expected failures: None

#### Daemon Tests (4 tests)
```bash
cargo test --lib daemon
```

Tests cover:
- ✅ Constants validation
- ✅ MIN/MAX delay relationships
- ✅ Channel capacity validation

Expected failures: None

#### Logging Tests (11 tests)
```bash
cargo test --lib logging
```

Tests cover:
- ✅ Log rotation for small and large files
- ✅ Path generation (Windows/Unix)
- ✅ Log level parsing
- ✅ Multiple rotation cycles

Expected failures: None (some tests may skip on permissions issues)

#### State Tests (19 tests)
```bash
cargo test --lib state
```

Tests cover:
- ✅ State file path generation
- ✅ State loading and saving
- ✅ Duration formatting (all time ranges)
- ✅ JSON serialization/deserialization
- ✅ Timestamp edge cases

Expected failures: None

## Test Output Validation

### Success Indicators

When tests pass, you should see:
```
test result: ok. 65 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Expected Warnings

Some tests may generate warnings (these are normal):
- File system operations in temporary directories
- Environment variable manipulation

## Platform-Specific Considerations

### Windows
- Event log tests require administrative privileges
- Path tests use Windows-specific structures (`APPDATA`)

### Unix/Linux/macOS
- Path tests use Unix-specific structures (`~/.local/share`)
- Signal handling tests are Unix-specific

## Debugging Failed Tests

If tests fail, use these commands:

```bash
# Run tests with output
cargo test --lib -- --nocapture

# Run a specific test
cargo test --lib test_extract_captive_portal_url_valid

# Run tests with backtrace
RUST_BACKTRACE=1 cargo test --lib
```

## Performance Considerations

- Most tests complete in milliseconds
- Log rotation tests may take longer (file I/O)
- Network-related tests are not included (would require mocking)

## Coverage Gaps

The following areas require integration/mocking frameworks:

1. **Network Functions** (captive_portal.rs):
   - `login()` - requires HTTP mocking
   - `logout()` - requires HTTP mocking
   - `verify_internet_connectivity()` - requires HTTP mocking
   - `check_captive_portal()` - requires HTTP mocking

2. **Daemon Loop** (daemon.rs):
   - `run()` - requires network watcher mocking
   - `run_with_credentials()` - requires time manipulation
   - `run_with_shutdown()` - Windows-specific, requires service mocking

3. **Platform-Specific** (logging.rs):
   - `register_event_log()` - Windows only
   - `deregister_event_log()` - Windows only

## Integration Test Recommendations

For future work, consider adding integration tests:

```rust
#[cfg(test)]
mod integration_tests {
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_login_with_mock_server() {
        let mock_server = MockServer::start().await;
        // Mock HTTP responses
        // Test login flow
    }
}
```

## Continuous Integration

Add to your CI pipeline:

```yaml
# .github/workflows/test.yml
- name: Run tests
  run: cargo test --lib --verbose

- name: Run tests with coverage
  run: cargo tarpaulin --lib --out Xml
```

## Maintenance

When modifying the codebase:

1. **Add tests first** (TDD approach)
2. **Run tests before committing**: `cargo test --lib`
3. **Update this document** if test structure changes
4. **Maintain test naming**: `test_<function>_<scenario>`

## Test Metrics

Current coverage (unit tests only):

| Module           | Functions Tested | Lines Covered | Edge Cases |
|------------------|------------------|---------------|------------|
| captive_portal   | 6/10 (60%)      | ~150 lines    | 31 tests   |
| daemon           | Constants only   | ~20 lines     | 4 tests    |
| logging          | 3/4 (75%)       | ~80 lines     | 11 tests   |
| state            | 4/4 (100%)      | ~97 lines     | 19 tests   |

**Total: 65 unit tests covering core functionality**

## Contact

For issues with tests:
1. Check this document first
2. Review TEST_COVERAGE_SUMMARY.md
3. Run with `--nocapture` for detailed output
4. File an issue with test output

---

Last updated: 2026-02-04