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

## 性能

测试数据：`package.json`（17011 字节） / `package.pln`（13074 字节，76.9%）

| 操作 | serde_json | popline-rust | 比 |
|------|-----------|-------------|------|
| 解析 | — | — | 待基准测试 |
| 序列化 | — | — | 待基准测试 |

## 测试

```bash
cargo test
```
