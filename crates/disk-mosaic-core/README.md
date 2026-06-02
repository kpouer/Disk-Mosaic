# disk-mosaic-core

The core library of Disk Mosaic, providing the fundamental functionalities for disk analysis and file structure data management.

## Features

- **Directory Scanning**: Recursive file system analysis to calculate sizes.
- **Data Model**: Structures to represent file and folder hierarchy.
- **Treemap Calculation**: Integration with the `treemap` crate for spatial visualization.
- **Performance**: Uses `rayon` to parallelize scanning operations.

## Usage

This crate is used as a base by `disk-mosaic-gui` and `disk-mosaic-tui`.

```rust
// Conceptual example
use disk_mosaic_core::directory_scanner::ScanConfig;
// ...
```
