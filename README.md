# chrono-diagnostics

Rust 结构化错误、运行日志和按创建期限滚动的文件日志。

运行 `VERUS=/path/to/verus ./check.sh` 构建并测试此库，执行生产和测试中的全部 Verus 证明。工具版本见 `rust-toolchain.toml` 与检查脚本。编译和验证产物在本次命令结束时清理。Cargo 使用方可按仓库 URL 和固定提交引用 `chrono-diagnostics`。

源码和测试取自 [ChronoAIProject/chrono-harness 的固定提交](https://github.com/ChronoAIProject/chrono-harness/tree/5f6892dfa2f73675a84e4ebdc3b06eb83b824d6e/crates/diagnostics)。
