# PopLine Rust

PopLine 序列化格式的 Rust 实现。

## Cargo.toml

```toml
[dependencies]
pln = "0.4"
```

## 使用

```rust
use pln::{from_str, to_string};

// PopLine → Rust 对象
let v = from_str("{\nkey: \"value\"\n").unwrap();

// Rust 对象 → PopLine
let s = to_string(&v);
```

## 性能

测试数据：`test.json`（17011 B）→ `test.pln`（13076 B，**76.9%**），5000 次迭代

| 操作 | serde_json | pln | 比 |
|------|-----------|-------------|------|
| 解析 | 149 µs/op | 152 µs/op | **1.02x** |
| 序列化 | 28 µs/op | 32 µs/op | **1.14x** |

## 测试

```bash
cargo test
```

## 致谢

本项目的开发得到了以下 AI 工具的大力协助：
- [DeepSeek](https://deepseek.com)（深度求索）
