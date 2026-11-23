# chrono-diagnostics

Rust 结构化错误、运行日志和按创建期限滚动的文件日志。

运行 `VERUS=/path/to/verus ./check.sh` 构建并测试此库，执行生产和测试中的全部 Verus 证明。工具版本见 `rust-toolchain.toml` 与检查脚本。每次检查在当前检出的 `target/check.XXXXXXXX` 下创建独立目录，存放 Cargo 编译产物、Verus 结果和子进程临时文件。检查退出时删除该目录。若进程被强制终止，残留随宿主回收该工作树。长期主检出需自行管理中断留下的目录。显式设置 `CARGO_TARGET_DIR` 可复用调用方负责保留和回收的 Cargo 缓存。Cargo 使用方可按仓库 URL 和固定提交引用 `chrono-diagnostics`。

源码和测试取自 [ChronoAIProject/chrono-harness 的固定提交](https://github.com/ChronoAIProject/chrono-harness/tree/5f6892dfa2f73675a84e4ebdc3b06eb83b824d6e/crates/diagnostics)。
