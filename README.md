# chrono-diagnostics

Rust 结构化错误、运行日志和按创建期限滚动的文件日志。

运行 `VERUS=/path/to/verus ./check.sh` 构建并测试此库，执行生产和测试中的全部 Verus 证明。工具版本见 `rust-toolchain.toml` 与检查脚本。每次检查在当前检出的 `target/check.XXXXXXXX` 下创建独立目录，存放 Cargo 编译产物、Verus 结果和子进程临时文件。检查退出时删除该目录。若进程被强制终止，残留随宿主回收该工作树。长期主检出需自行管理中断留下的目录。显式设置 `CARGO_TARGET_DIR` 可复用调用方负责保留和回收的 Cargo 缓存。Cargo 使用方可按仓库 URL 和固定提交引用 `chrono-diagnostics`。

源码和测试取自 [ChronoAIProject/chrono-harness 的固定提交](https://github.com/ChronoAIProject/chrono-harness/tree/5f6892dfa2f73675a84e4ebdc3b06eb83b824d6e/crates/diagnostics)。

## 定时回收日志

从本仓 Release 下载当前平台的 `chrono-diagnostics`，按同一 Release 的 `SHA256SUMS` 校验后安装到工作树外。宿主可运行 `chrono-diagnostics expire-logs-v1 ROOT` 回收该检出的到期日志，程序读取现有 `.chrono-harness/logging.json`。没有时间保留策略时，stdout 为 JSON `null`；否则返回 `removed`、`legacy`、`busy` 和 `errors`。成功或写入者正持锁时退出 0，单项删除失败时退出 1 并保留完整 JSON。配置、读取或结果发布失败时退出 2，诊断写入 stderr。

## 记录独立命令的运行日志

启用日志的命令可以调用 `chrono-diagnostics cli-events-v1 ROOT PRODUCER OPERATION PID`。传入实际业务进程的 PID。读取标准输出的一行 `{"ready":true}` 后开始工作。业务可向其标准输入写入 `{"diagnostic":"原始错误"}`，最后写入 `{"exit_code":N}` 并等待子进程结束。每条消息都是单个 JSON 对象。业务的标准输出由业务命令直接交给使用方。日志使用宿主的级别、目录、滚动和过期策略。成功发布日志时返回业务退出码，输入损坏、缺少终态或日志发布失败时返回 2。
