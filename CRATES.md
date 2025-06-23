# Rew Runtime Crates

This project has been refactored into multiple modular crates to improve maintainability and separation of concerns.

## Crate Structure

### Core Crates

- **`rew-runtime`** - Main runtime that orchestrates all other crates
- **`rew-fs`** - Filesystem operations (reading, writing, directory operations)
- **`rew-data`** - Data management operations (storage, retrieval, formats)
- **`rew-os`** - Operating system information and terminal operations
- **`rew-utils`** - Utility functions (base64, YAML, random generation, virtual files)

### Existing Crates

- **`rew-qrew`** - QRew functionality
- **`rew_bindgen_macros`** - Bindgen macros
- **`rew-qrew-stub`** - QRew stub
- **`rew_bindgen`** - Bindgen functionality

## Benefits of Modular Structure

1. **Separation of Concerns**: Each crate has a specific responsibility
2. **Maintainability**: Easier to maintain and update individual components
3. **Testing**: Each crate can be tested independently
4. **Reusability**: Individual crates can be used in other projects
5. **Compilation**: Faster incremental builds when only specific functionality changes

## Usage

The main entry point remains the same. The `src/runtime.rs` file now simply re-exports the new modular runtime:

```rust
// Re-export the new modular runtime
pub use rew_runtime::*;
```

## Development

When developing, you can work on individual crates:

```bash
# Work on filesystem operations
cd rew-fs
cargo test

# Work on data operations  
cd rew-data
cargo test

# Work on the main runtime
cd rew-runtime
cargo test
```

## Building

The workspace is configured to build all crates together:

```bash
# Build all crates
cargo build

# Build specific crate
cargo build -p rew-fs
```

