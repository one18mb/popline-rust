# PopLine Rust

Rust crate for the PopLine serialization format.

## Cargo.toml

```toml
[dependencies]
popline-rust = "0.1.0"
```

## Usage

```rust
use popline_rust::{parse, serialize, PlnValue};

// Parse
let v = parse("{\nkey: \"value\"\n").unwrap();

// Serialize
let s = serialize(&v);

// Build DOM
let mut obj = PlnValue::new_object();
obj.add_to_object("name", PlnValue::new_string("test"));
```

## Performance

Data: `package.json` (17011 B) / `package.pln` (13074 B, 76.9%)

| Operation | serde_json | popline-rust | Ratio |
|-----------|-----------|-------------|-------|
| Parse | — | — | TBD |
| Serialize | — | — | TBD |

## Test

```bash
cargo test
```
