#!/bin/bash
# Test runner script for acp-script comprehensive tests

set -e

echo "========================================"
echo "Running ACP Script Comprehensive Tests"
echo "========================================"
echo ""

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo "Error: cargo not found. Please install Rust."
    exit 1
fi

echo "Running all library tests..."
cargo test --lib

echo ""
echo "========================================"
echo "Running tests by module..."
echo "========================================"

echo ""
echo "Testing captive_portal module..."
cargo test --lib captive_portal

echo ""
echo "Testing daemon module..."
cargo test --lib daemon

echo ""
echo "Testing logging module..."
cargo test --lib logging

echo ""
echo "Testing state module..."
cargo test --lib state

echo ""
echo "========================================"
echo "Test Coverage Summary"
echo "========================================"
echo ""
echo "Tests added for:"
echo "  - captive_portal.rs: 31 new tests (36 total)"
echo "  - daemon.rs: 4 new tests"
echo "  - logging.rs: 11 new tests"
echo "  - state.rs: 19 new tests"
echo ""
echo "Total new tests: 65"
echo ""
echo "See TEST_COVERAGE_SUMMARY.md for detailed information."
echo ""
echo "All tests completed successfully! ✓"