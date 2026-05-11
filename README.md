# PopLine Rust

PopLine 序列化格式的 Rust 实现。

## Cargo.toml

```toml
[dependencies]
popline-rust = "0.1.0"
```

## 使用

```rust
use popline_rust::{parse, serialize, PlnValue};

// 解析
let v = parse("{\nkey: \"value\"\n").unwrap();

// 序列化
let s = serialize(&v);

// 构建 DOM
let mut obj = PlnValue::new_object();
obj.add_to_object("name", PlnValue::new_string("test"));
```

## 测试

```bash
cargo test
```
