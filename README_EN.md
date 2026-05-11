# PopLine Rust

Rust crate for the PopLine serialization format.

## Cargo.toml

```toml
[dependencies]
pln = "0.1.0"
```

## Usage

```rust
use pln::{from_str, to_string, PlnValue};

// Parse
let v = from_str("{\nkey: \"value\"\n").unwrap();

// Serialize
let s = to_string(&v);

// Build DOM
let mut obj = PlnValue::new_object();
obj.add_to_object("name", PlnValue::new_string("test"));
```

## Performance

Data: `package.json` (17011 B) / `package.pln` (13074 B, 76.9%)

| Operation | serde_json | pln | Ratio |
|-----------|-----------|-------------|-------|
| Parse | — | — | TBD |
| Serialize | — | — | TBD |

## Test

```bash
cargo test
```
