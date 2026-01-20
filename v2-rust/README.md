# Progred - Graph Editor

A native graph editor for working with graph structures, built with Rust and egui.

## Architecture

- **UI**: egui (immediate mode GUI)
- **Data structures**: Immutable persistent data structures (`im` crate)
- **Incremental computation**: salsa for automatic recomputation of derived values

## Development

### Prerequisites

- Rust (latest stable)

### Running

```bash
cargo run
```

### Building Release

```bash
cargo build --release
```

## Features (Planned)

- Tree view for exploring graph structure
- Inline editing with autocomplete
- Graph visualization with nodes and edges
- Projections for different views of the data
- Bootstrap visualization for understanding circular definitions
