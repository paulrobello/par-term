# Makefile for par-term
# Cross-platform terminal emulator frontend

.PHONY: help build build-debug run run-release run-error run-warn run-info run-debug run-trace release test check clean fmt lint checkall install doc coverage test-fonts benchmark-shaping test-text-shaping bundle bundle-install run-bundle

# Default target
.DEFAULT_GOAL := help

# Help target - display available commands
help:
	@echo "par-term - Cross-platform Terminal Emulator"
	@echo ""
	@echo "Available targets:"
	@echo "  make build       - Build the project in release mode"
	@echo "  make build-debug - Build the project in debug mode"
	@echo "  make run         - Run the application (release mode)"
	@echo "  make run-debug   - Run with debug logging"
	@echo ""
	@echo "Run with logging:"
	@echo "  make run-error   - Run with error level logs"
	@echo "  make run-warn    - Run with warning level logs"
	@echo "  make run-info    - Run with info level logs"
	@echo "  make run-perf    - Run with performance logging to /tmp/par-term-perf.log"
	@echo "  make run-debug   - Run with DEBUG_LEVEL=3 (logs to /tmp/par_term_debug.log)"
	@echo "  make run-trace   - Run with DEBUG_LEVEL=4 (most verbose)"
	@echo ""
	@echo "Text Shaping Testing:"
	@echo "  make test-fonts        - Run comprehensive text shaping test suite"
	@echo "  make benchmark-shaping - Run text shaping performance benchmark"
	@echo "  make test-text-shaping - Run both font tests and benchmark"
	@echo ""
	@echo "Graphics Testing:"
	@echo "  make test-graphics     - Test graphics with debug logging"
	@echo "  make test-animations   - Test Kitty animations"
	@echo "  make tail-log          - Monitor debug log in real-time"
	@echo "  make watch-graphics    - Monitor graphics-related logs only"
	@echo "  make show-graphics-logs - Show recent graphics logs"
	@echo "  make clean-logs        - Clean debug logs"
	@echo ""
	@echo "Testing & Quality:"
	@echo "  make test        - Run all tests"
	@echo "  make check       - Check code without building"
	@echo "  make fmt         - Format code using rustfmt"
	@echo "  make lint        - Run clippy linter"
	@echo "  make checkall    - Format, lint, and test"
	@echo "  make all         - Format, lint, test, and build"
	@echo ""
	@echo "macOS Bundle:"
	@echo "  make bundle         - Create macOS .app bundle (release mode)"
	@echo "  make bundle-install - Install .app bundle to /Applications and binary to PATH"
	@echo "  make run-bundle     - Run as macOS .app (shows dock icon)"
	@echo ""
	@echo "Other:"
	@echo "  make clean       - Clean build artifacts"
	@echo "  make install     - Install the binary"
	@echo "  make doc         - Generate and open documentation"
	@echo "  make coverage    - Generate test coverage report"
	@echo ""

# Build in release mode (default for faster runtime)
build:
	@echo "Building par-term (release mode)..."
	cargo build --release

# Build in debug mode
build-debug:
	@echo "Building par-term (debug mode)..."
	cargo build

# Alias for build (release mode)
release: build
	@echo "Release binary: target/release/par-term"

# Run the application (always use release mode for performance)
run:
	@echo "Running par-term (release mode)..."
	cargo run --release

# Run the application in release mode
run-release:
	@echo "Running par-term (release mode)..."
	cargo run --release

# Run with error level logging
run-error:
	@echo "Running par-term (error level logs)..."
	RUST_LOG=error cargo run

# Run with warning level logging
run-warn:
	@echo "Running par-term (warn level logs)..."
	RUST_LOG=warn cargo run

# Run with info level logging (default)
run-info:
	@echo "Running par-term (info level logs)..."
	RUST_LOG=info cargo run

# Run with performance logging to file
run-perf:
	@echo "Running par-term with performance logging..."
	@echo "Logs will be written to: /tmp/par-term-perf.log"
	@echo ""
	@echo "💡 In another terminal, run:"
	@echo "   tail -f /tmp/par-term-perf.log | grep PERF"
	@echo ""
	RUST_LOG=info cargo run --release 2>&1 | tee /tmp/par-term-perf.log

# Run with debug level logging (uses custom DEBUG_LEVEL for file logging)
run-debug:
	@echo "Running par-term with DEBUG_LEVEL=3..."
	@echo "Debug log: /tmp/par_term_debug.log"
	@echo ""
	@echo "💡 In another terminal, run: make tail-log"
	@echo ""
	RUST_LOG=debug DEBUG_LEVEL=3 cargo run

# Run with trace level logging (most verbose, uses custom DEBUG_LEVEL)
run-trace:
	@echo "Running par-term with DEBUG_LEVEL=4 (trace)..."
	@echo "Debug log: /tmp/par_term_debug.log"
	@echo ""
	@echo "💡 In another terminal, run: make tail-log"
	@echo ""
	RUST_LOG=trace DEBUG_LEVEL=4 cargo run

# Run release build with debug logging
run-release-debug: release
	@echo "Running release build with DEBUG_LEVEL=3..."
	@echo "Debug log: /tmp/par_term_debug.log"
	@echo ""
	@echo "💡 In another terminal, run: make tail-log"
	@echo ""
	RUST_LOG=debug DEBUG_LEVEL=3 ./target/release/par-term

# Run all tests
test:
	@echo "Running tests..."
	cargo test

# Run tests with output
test-verbose:
	@echo "Running tests (verbose)..."
	cargo test -- --nocapture

# Run specific test
test-one:
	@echo "Running specific test..."
	@echo "Usage: make test-one TEST=test_name"
	cargo test $(TEST)

# Check code without building
check:
	@echo "Checking code..."
	cargo check

# Check all targets
check-all:
	@echo "Checking all targets..."
	cargo check --all-targets

# Format code
fmt:
	@echo "Formatting code..."
	cargo fmt

# Check formatting without modifying files
fmt-check:
	@echo "Checking code formatting..."
	cargo fmt -- --check

# Run clippy linter
lint:
	@echo "Running clippy..."
	cargo clippy -- -D warnings

# Run clippy on all targets
lint-all:
	@echo "Running clippy on all targets..."
	cargo clippy --all-targets -- -D warnings

# Run all quality checks (format, lint, test)
checkall: fmt lint test
	@echo "All quality checks passed!"

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	cargo clean

# Install the binary
install:
	@echo "Installing par-term..."
	cargo install --path .

# Generate documentation
doc:
	@echo "Generating documentation..."
	cargo doc --no-deps

# Generate and open documentation
doc-open:
	@echo "Generating and opening documentation..."
	cargo doc --no-deps --open

# Run all checks (format, lint, test)
all: fmt lint test build
	@echo "All checks passed!"

# Pre-commit checks
pre-commit: fmt-check lint test
	@echo "Pre-commit checks passed!"

# CI checks (what CI would run)
ci: fmt-check lint-all test check-all
	@echo "CI checks passed!"

# Update dependencies
update:
	@echo "Updating dependencies..."
	cargo update

# Generate test coverage (requires tarpaulin)
coverage:
	@echo "Generating test coverage..."
	@command -v cargo-tarpaulin >/dev/null 2>&1 || { echo "cargo-tarpaulin not installed. Install with: cargo install cargo-tarpaulin"; exit 1; }
	cargo tarpaulin --out Html --output-dir coverage

# Benchmark (when benchmarks are added)
bench:
	@echo "Running benchmarks..."
	cargo bench

# Watch for changes and rebuild
watch:
	@echo "Watching for changes..."
	@command -v cargo-watch >/dev/null 2>&1 || { echo "cargo-watch not installed. Install with: cargo install cargo-watch"; exit 1; }
	cargo watch -x check -x test

# Watch and run
watch-run:
	@echo "Watching and running..."
	@command -v cargo-watch >/dev/null 2>&1 || { echo "cargo-watch not installed. Install with: cargo install cargo-watch"; exit 1; }
	cargo watch -x run

# Audit dependencies for security vulnerabilities
audit:
	@echo "Auditing dependencies..."
	@command -v cargo-audit >/dev/null 2>&1 || { echo "cargo-audit not installed. Install with: cargo install cargo-audit"; exit 1; }
	cargo audit

# Show package info
info:
	@echo "Package information:"
	@cargo metadata --no-deps --format-version 1 | grep -E '"name"|"version"|"authors"|"description"'

# Create release build and package
package: release
	@echo "Creating release package..."
	@mkdir -p dist
	@cp target/release/par-term dist/
	@cp README.md LICENSE-MIT dist/
	@echo "Package created in dist/"

# macOS app bundle
bundle: release
ifeq ($(shell uname),Darwin)
	@echo "Creating macOS app bundle..."
	$(eval VERSION := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/'))
	@mkdir -p target/release/bundle/par-term.app/Contents/MacOS
	@mkdir -p target/release/bundle/par-term.app/Contents/Resources
	@cp target/release/par-term target/release/bundle/par-term.app/Contents/MacOS/
	@cp assets/par-term.icns target/release/bundle/par-term.app/Contents/Resources/
	@echo '<?xml version="1.0" encoding="UTF-8"?>' > target/release/bundle/par-term.app/Contents/Info.plist
	@echo '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '<plist version="1.0">' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '<dict>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <key>CFBundleName</key>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <string>par-term</string>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <key>CFBundleDisplayName</key>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <string>par-term</string>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <key>CFBundleIdentifier</key>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <string>com.paulrobello.par-term</string>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <key>CFBundleVersion</key>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <string>$(VERSION)</string>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <key>CFBundleShortVersionString</key>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <string>$(VERSION)</string>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <key>CFBundleExecutable</key>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <string>par-term</string>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <key>CFBundleIconFile</key>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <string>par-term</string>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <key>CFBundlePackageType</key>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <string>APPL</string>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <key>NSHighResolutionCapable</key>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <true/>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <key>LSMinimumSystemVersion</key>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '    <string>10.13</string>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '</dict>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo '</plist>' >> target/release/bundle/par-term.app/Contents/Info.plist
	@echo "Bundle created at: target/release/bundle/par-term.app (version $(VERSION))"
else
	@echo "App bundle creation is only supported on macOS"
endif

# Run macOS app bundle (shows proper dock icon)
run-bundle: bundle
ifeq ($(shell uname),Darwin)
	@echo "Running par-term.app..."
	@open target/release/bundle/par-term.app
else
	@echo "App bundle is only supported on macOS"
	@echo "Running regular binary instead..."
	cargo run --release
endif

# Install macOS app bundle to /Applications and binary to PATH
bundle-install: bundle install
ifeq ($(shell uname),Darwin)
	@echo "Installing par-term.app to /Applications..."
	@if [ -d "/Applications/par-term.app" ]; then \
		echo "Removing existing /Applications/par-term.app..."; \
		rm -rf /Applications/par-term.app; \
	fi
	@cp -R target/release/bundle/par-term.app /Applications/
	@echo "✅ Installed to /Applications/par-term.app"
else
	@echo "App bundle installation is only supported on macOS"
endif

# Generate config file example
config-example:
	@echo "# par-term configuration example" > config.yaml.example
	@echo "cols: 80" >> config.yaml.example
	@echo "rows: 24" >> config.yaml.example
	@echo "font_size: 14.0" >> config.yaml.example
	@echo "font_family: \"JetBrains Mono\"" >> config.yaml.example
	@echo "scrollback_size: 10000" >> config.yaml.example
	@echo "window_title: \"par-term\"" >> config.yaml.example
	@echo "theme: \"dark-background\"" >> config.yaml.example
	@echo "auto_copy_selection: false" >> config.yaml.example
	@echo "middle_click_paste: true" >> config.yaml.example
	@echo "screenshot_format: \"png\"" >> config.yaml.example
	@echo "Example config written to config.yaml.example"

# === Text Shaping Testing Targets ===

# Run comprehensive text shaping test suite
test-fonts:
	@echo "🔤 Running comprehensive font and text shaping tests..."
	@echo ""
	@echo "This test suite covers:"
	@echo "  ✓ Emoji with skin tones"
	@echo "  ✓ Flag emoji (Regional Indicators)"
	@echo "  ✓ ZWJ sequences"
	@echo "  ✓ Complex scripts (Arabic, Devanagari, Thai)"
	@echo "  ✓ BiDi text (LTR + RTL)"
	@echo "  ✓ Combining diacritics"
	@echo "  ✓ Programming ligatures"
	@echo "  ✓ Wide character rendering"
	@echo ""
	@./scripts/test_fonts.sh

# Run text shaping performance benchmark
benchmark-shaping:
	@echo "⚡ Running text shaping performance benchmark..."
	@echo ""
	@echo "This will benchmark:"
	@echo "  - ASCII baseline"
	@echo "  - CJK characters"
	@echo "  - Simple and complex emoji"
	@echo "  - Complex scripts (Arabic, Devanagari, Thai)"
	@echo "  - Mixed content stress test"
	@echo ""
	@echo "💡 Run this twice to compare:"
	@echo "   1. With enable_text_shaping: true (default)"
	@echo "   2. With enable_text_shaping: false"
	@echo ""
	@./scripts/benchmark_text_shaping.sh

# Run both font tests and benchmark
test-text-shaping: test-fonts benchmark-shaping
	@echo ""
	@echo "✅ Text shaping tests and benchmarks complete!"

# === Graphics Testing Targets ===

# Monitor the debug log
tail-log:
	@if [ ! -f /tmp/par_term_debug.log ]; then \
		echo "❌ Debug log not found."; \
		echo ""; \
		echo "Start par-term with: make run-debug"; \
		exit 1; \
	fi
	@echo "📝 Monitoring /tmp/par_term_debug.log..."
	@echo "Press Ctrl+C to stop"
	@echo ""
	tail -f /tmp/par_term_debug.log

# Watch log with graphics filtering
watch-graphics:
	@if [ ! -f /tmp/par_term_debug.log ]; then \
		echo "❌ Debug log not found."; \
		echo ""; \
		echo "Start par-term with: make run-debug"; \
		exit 1; \
	fi
	@echo "📝 Monitoring /tmp/par_term_debug.log (graphics only)..."
	@echo "Press Ctrl+C to stop"
	@echo ""
	tail -f /tmp/par_term_debug.log | grep -i --line-buffered "graphics\|terminal\|animation\|sixel\|kitty"

# Test graphics with debug logging
test-graphics:
	@echo "🎨 Graphics Testing Mode"
	@echo ""
	@echo "Starting par-term with DEBUG_LEVEL=4..."
	@echo ""
	@echo "📝 Debug log: /tmp/par_term_debug.log"
	@echo ""
	@echo "💡 In another terminal, run:"
	@echo "   make tail-log         (all logs)"
	@echo "   make watch-graphics   (graphics only)"
	@echo ""
	@echo "🧪 In par-term, run test:"
	@echo "   bash /tmp/test_par_term_graphics.sh"
	@echo ""
	DEBUG_LEVEL=4 cargo run

# Clean debug logs
clean-logs:
	@echo "Cleaning debug logs..."
	@rm -f /tmp/par_term_debug.log
	@echo "✅ Debug logs cleaned"

# Show recent graphics-related logs
show-graphics-logs:
	@if [ ! -f /tmp/par_term_debug.log ]; then \
		echo "❌ Debug log not found."; \
		echo ""; \
		echo "Start par-term with: make run-debug"; \
		exit 1; \
	fi
	@echo "Recent graphics-related logs:"
	@echo ""
	@grep -i "graphics\|terminal\|animation\|sixel" /tmp/par_term_debug.log | tail -30

# Test animations specifically
test-animations:
	@echo "🎬 Animation Testing Mode"
	@echo ""
	@echo "Starting par-term with DEBUG_LEVEL=4..."
	@echo ""
	@echo "📝 Debug log: /tmp/par_term_debug.log"
	@echo ""
	@echo "💡 In another terminal, run: make watch-graphics"
	@echo ""
	@echo "🧪 In par-term, run:"
	@echo "   uv run python ../par-term-emu-core-rust/scripts/test_kitty_animation.py"
	@echo ""
	DEBUG_LEVEL=4 cargo run

# === Profiling Targets ===

# Profile with flamegraph (CPU profiling)
profile:
	@echo "🔥 Profiling with flamegraph..."
	@command -v cargo-flamegraph >/dev/null 2>&1 || { echo "❌ cargo-flamegraph not installed."; echo "Install with: cargo install flamegraph"; exit 1; }
	@echo ""
	@echo "This will:"
	@echo "  1. Build in release mode with debug symbols"
	@echo "  2. Run par-term and collect CPU samples"
	@echo "  3. Generate flamegraph.svg when you quit"
	@echo ""
	@echo "💡 Use the terminal normally, then quit to generate the flamegraph"
	@echo ""
	cargo flamegraph --release --output flamegraph.svg

# Profile with perf (Linux only)
profile-perf:
	@echo "📊 Profiling with perf..."
	@command -v perf >/dev/null 2>&1 || { echo "❌ perf not installed (Linux only)"; exit 1; }
	@echo "Recording 30 seconds of perf data..."
	@echo "Use the terminal normally during this time"
	cargo build --release
	perf record -F 99 -g --call-graph=dwarf -- target/release/par-term
	@echo ""
	@echo "Generating report..."
	perf report

# Profile with Instruments (macOS only)
profile-instruments:
	@echo "🎯 Profiling with Instruments (macOS)..."
	@command -v instruments >/dev/null 2>&1 || { echo "❌ Instruments not found (macOS only)"; exit 1; }
	cargo build --release
	@echo "Starting Instruments Time Profiler..."
	@echo "Use the terminal normally, then stop recording in Instruments"
	instruments -t "Time Profiler" target/release/par-term
