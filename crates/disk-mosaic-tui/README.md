# disk-mosaic-tui

The terminal user interface (TUI) for Disk Mosaic, built with `ratatui`.
This is a library used for Terminal User Interface of https://crates.io/crates/Disk-Mosaic

## Features

- **Lightweight**: Fast disk analysis directly in your terminal.
- **Text Visualization**: Displays disk usage as a textual treemap or lists.
- **Keyboard Shortcuts**: Quick keyboard-based navigation.

## Key Dependencies

- `ratatui`: Library for building terminal user interfaces.
- `crossterm`: Multi-platform terminal management.
- `disk-mosaic-core`: Shared scanning logic.

## How to use

This crate can be launched via the main workspace binary with the `tui` flag or via the `dm` binary.
