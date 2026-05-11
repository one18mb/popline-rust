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

测试数据：`package.json`（17011 B）→ `package.pln`（13074 B，**76.9%**），5000 次迭代

| 操作 | serde_json | pln | 比 |
|------|-----------|-------------|------|
| 解析 | 4788 ms (958 µs/op) | 9042 ms (1808 µs/op) | 1.89x |
| 序列化 | 6783 ms (1357 µs/op) | 2323 ms (465 µs/op) | **0.34x** |

## 测试

```bash
cargo test
```
