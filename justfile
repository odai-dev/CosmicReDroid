# Project Metadata
name := "cosmic-redroid"
appid := "com.system76.CosmicRedroid"

# Path Configuration
rootdir := ""
prefix := "/usr"
bindir := prefix / "bin"
datadir := prefix / "share"
iconsdir := datadir / "icons"
desktopdir := datadir / "applications"
metainfodir := datadir / "metainfo"

# Default task: compile release binary
default: all

# Build all release targets
all: build-release

# Build shorthand
build: build-release

# Compile debug build
build-debug:
    cargo build

# Compile optimized release build
build-release:
    cargo build --release

# Run locally in debug mode
run:
    cargo run

# Run locally in release mode
run-release:
    cargo run --release

# Run all unit and integration tests
test:
    cargo test

# Check code with cargo clippy
check:
    cargo clippy --all-targets -- -D warnings

# Install binary and desktop integration assets
install:
    install -Dm0755 "target/release/{{ name }}" "{{ rootdir }}{{ bindir }}/{{ name }}"
    install -Dm0644 "resources/app.desktop" "{{ rootdir }}{{ desktopdir }}/{{ appid }}.desktop"
    install -Dm0644 "resources/app.metainfo.xml" "{{ rootdir }}{{ metainfodir }}/{{ appid }}.metainfo.xml"
    install -Dm0644 "resources/icons/hicolor/scalable/apps/icon.svg" "{{ rootdir }}{{ iconsdir }}/hicolor/scalable/apps/{{ appid }}.svg"

# Uninstall binary and desktop integration assets
uninstall:
    rm -f "{{ rootdir }}{{ bindir }}/{{ name }}"
    rm -f "{{ rootdir }}{{ desktopdir }}/{{ appid }}.desktop"
    rm -f "{{ rootdir }}{{ metainfodir }}/{{ appid }}.metainfo.xml"
    rm -f "{{ rootdir }}{{ iconsdir }}/hicolor/scalable/apps/{{ appid }}.svg"

# Vendor dependencies for offline package builds
vendor:
    cargo vendor

# Clean cargo artifacts
clean:
    cargo clean
