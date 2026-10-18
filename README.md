# CosmicReDroid

Native Linux Android Multi-Instance Control Center built on System76 libcosmic and ReDroid containers.

## Features

- Multi-instance ReDroid management via Docker Bollard API
- Hardware identity spoofing (Galaxy S24 Ultra, Pixel 8 Pro, ASUS ROG Phone 8)
- Deterministic binder node isolation and DRI GPU acceleration
- Copy-on-Write (CoW) instance cloning and zstd snapshot engine
- Integrated Image Factory for custom Android image builds
- Host diagnostics with pkexec remediation
- Native COSMIC UI with zero emojis and FreeDesktop symbolic icons

## Building

Requires `cargo` and `just`.

```bash
just build
```

## Running

```bash
just run
```

## License

GPL-3.0-only
