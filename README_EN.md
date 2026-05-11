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

Data: `package.json` (17011 B) → `package.pln` (13074 B, **76.9%**), 5000 iterations

| Operation | serde_json | pln | Ratio |
|-----------|-----------|-------------|-------|
| Parse | 4788 ms (958 µs/op) | 9042 ms (1808 µs/op) | 1.89x |
| Serialize | 6783 ms (1357 µs/op) | 2323 ms (465 µs/op) | **0.34x** |

## Test

```bash
cargo test
```
