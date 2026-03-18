# DiskBlock

DiskBlock is a macOS terminal disk usage analyzer built with Rust, ratatui, crossterm, and rayon.

It visualizes directory contents as proportional rectangles. The current directory fills the main viewport, and children partition that area by size.

## Features

- Proportional recursive partition view with alternating split direction by depth
- Top-N largest children shown, remainder grouped into Other
- Lazy loading: children are scanned when you zoom in
- Background scanning with partial updates for responsive UI
- Stable lazy-loading layout: already loaded children keep their relative size while a Loading slice shrinks as data arrives
- Files are rendered as rectangles (not only directories)
- Very small entries are grouped into Other based on percent-of-parent threshold
- Other rectangle can show an inline list of aggregated file names
- Directory and file rectangles are visually distinct: directories are filled, files are outline-first
- Loading directories show a loading marker in the rectangle title, and unresolved space appears as a Loading slice
- Geometric sibling navigation using arrow keys (nearest rectangle center)
- Sudo prompt when entering protected directories
- Copy selected path to clipboard
- Breadcrumb bar and bottom info panel with size percentages

## Requirements

- macOS
- Rust toolchain (cargo + rustc)
- Terminal with UTF-8 support
- Optional for protected folders: sudo access

## Install

1. Clone the repository.
2. Build the project.

```bash
cargo build --release
```

## Run

Run from source in debug mode:

```bash
cargo run -- /
```

Run the release binary:

```bash
./target/release/diskblock /
```

You can replace / with any starting path.

Examples:

```bash
cargo run -- ~/Library
cargo run -- /System
```

## How To Use

### Navigation

- Up Arrow: move selection to nearest sibling above
- Down Arrow: move selection to nearest sibling below
- Left Arrow: move selection to nearest sibling on the left
- Right Arrow: move selection to nearest sibling on the right
- j: move to next sibling by index
- k: move to previous sibling by index
- Enter: zoom into selected directory
- l: zoom into selected directory
- h: go to parent directory
- u: go to parent directory
- Backspace: go to parent directory
- q: quit app

### Actions

- c: copy selected full path to clipboard
- ?: toggle help overlay

### Protected Directories

If a selected directory is not readable by the current user, DiskBlock temporarily leaves the alternate screen and asks for sudo authentication.

After successful authentication, scanning continues and sudo timestamp caching is handled by the system.

## UI Layout

- Top bar: breadcrumb path
- Main area: partitioned rectangle visualization
- Bottom bar: selected path, size, percent of parent, percent of root, percent of disk, status

## Colors

DiskBlock uses stable visual semantics:

- Directories: filled style
- Files: outline-first style
- Other: aggregated tiny-items rectangle with optional inline list

## Small Items And Other

- DiskBlock includes both directories and files in partitioning.
- Items that are below a small percent of their parent are grouped into Other.
- If Other contains many tiny files, the Other rectangle shows a compact inline list of file names.

## Data Source And Why Numbers Can Differ

- DiskBlock values are `du` path-based and may differ from macOS Storage category totals.
- DiskBlock measures per-path filesystem usage (directory tree view).
- macOS Storage Settings reports category accounting (for example Applications/System Data) that can include data spread across multiple paths and internal category rules.

## Math And Algorithm Definition

DiskBlock computes and renders values with the following definitions:

1. Size source

- For each child entry in the current directory, DiskBlock measures size with `du -sk <path>` (or `sudo -n du -sk <path>` when privileged).
- Sizes are converted to bytes as `size_bytes = size_kb * 1024`.

2. Parent total

- For a rendered directory level, the parent size is the sum of visible child sizes plus any unresolved Loading remainder while scanning.
- Once a directory total is known, it is locked to keep child proportions stable during lazy updates.

3. Percentages in the info panel

- `% parent = selected_size / parent_size * 100`
- `% root = selected_size / root_size * 100`
- `% disk = selected_size / disk_total * 100`, where `disk_total` comes from `df -k <root_path>`.

4. Rectangle partitioning

- Children are sorted by size descending.
- Small entries can be grouped into `Other`.
- Slice-and-dice layout assigns pixel span proportionally:
	- `span_i = floor(size_i / total_size * parent_span)` for non-last items.
	- Last item gets remaining span to guarantee exact fill and no gaps.

5. Rounding behavior

- Because terminal layout is integer pixels, tiny proportional differences are rounded.
- The last-slice remainder rule preserves exact partition coverage.

## Performance Notes

- Initial view scans the current directory and renders partial results while scanning continues
- Deep traversal is delayed until zoom-in to keep startup responsive
- Directory size work runs in parallel using rayon

## Testing

Run checks and tests:

```bash
cargo check
cargo test
```

Current layout tests include:

- Exact partition fill
- No-overlap assertions
- Proportional area tolerance assertions

## Known v1 Scope

- No deletion actions in safe mode yet
- No JSON export yet

## Project Structure

- src/scanner.rs: filesystem traversal and scan worker
- src/layout.rs: partitioning algorithm and layout tests
- src/ui.rs: rendering and panels
- src/app.rs: state and navigation
- src/actions.rs: clipboard and sudo prompt helpers
- src/main.rs: terminal loop and key handling
