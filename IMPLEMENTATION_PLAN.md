# DiskBlock Implementation Plan

## Goal
Build a high-performance macOS Rust TUI disk usage analyzer using ratatui + crossterm + rayon.

## Decisions
- Sudo flow: allow normal sudo cache window.
- Startup scan: hybrid (quick initial children + lazy deep scans on zoom).
- v1 actions: no delete; add copy-full-path action.
- v1 output: TUI only.

## Phases
1. Bootstrap crate structure and dependencies.
2. Build scanner with parallel traversal and background updates.
3. Implement slice-and-dice layout with exact parent fill.
4. Implement app state and keyboard navigation.
5. Implement TUI panels, rectangle rendering, breadcrumb, and info bar.
6. Add macOS category detection and privileged scan flow.
7. Add tests for layout correctness and scanner behavior.

## Acceptance
- Runs with `cargo run -- /`.
- Visualizes current directory as full viewport with proportional child partitions.
- Navigation: arrows + h/j/k/l, Enter/l zoom in, h parent, q quit.
- Uses lazy loading and stays responsive while scanning.
- Protected directories can be explored after sudo prompt.
- Copy selected path action available.
