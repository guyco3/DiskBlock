# memblocks

[![CI](https://github.com/guyco3/memblocks/actions/workflows/ci.yml/badge.svg)](https://github.com/guyco3/memblocks/actions/workflows/ci.yml)

memblocks is a macOS terminal disk usage TUI written in Rust.
It renders the current directory as a treemap-like partition where every immediate child (directories and files) is shown proportionally by size.

![memblocks example](static/example.jpg)

## Requirements

- macOS
- Rust toolchain (`cargo`, `rustc`)
- UTF-8 terminal
- Optional: `sudo` access for protected paths

## Quick Start

Build:

```bash
cargo build --release
```

Run from source:

```bash
cargo run -- /
```

Run release binary:

```bash
./target/release/memblocks/
```

You can replace `/` with any start path, for example:

```bash
cargo run -- ~/Library
cargo run -- /System
```

## Keybindings

- `q`: quit
- Arrow keys: geometric movement between neighboring rectangles
- `j` / `k`: next / previous item
- `Enter` or `l`: enter selected directory
- `h`, `u`, `Backspace`: go to parent directory
- `c`: copy selected path to clipboard
- `?`: toggle help

## Data Model

- Size source: `du -sk <path>` (or `sudo -n du -sk <path>` when privileged)
- Display unit: bytes (`KB * 1024`)
- Percentages shown:
	- `% parent = selected_size / parent_size * 100`
	- `% root = selected_size / root_size * 100`
	- `% disk = selected_size / disk_total * 100`
- Disk total source: `df -k <root_path>`

## Build And Test

```bash
cargo check
cargo test
```

## Project Layout

- `src/main.rs`: terminal lifecycle and key handling
- `src/app.rs`: application state and navigation behavior
- `src/scanner.rs`: filesystem scanning and watch invalidation
- `src/layout.rs`: rectangle partition algorithm and tests
- `src/ui.rs`: terminal rendering
- `src/actions.rs`: clipboard and sudo auth helpers
- `src/types.rs`: shared data structures

## Contributing

Issues and pull requests are welcome.

Suggested contributor flow:

1. Fork and create a focused branch.
2. Keep behavior changes covered by tests where possible.
3. Run `cargo check` and `cargo test` before opening a PR.
4. Include a clear summary and rationale in the PR description.

## License

MIT. See [LICENSE](LICENSE).
