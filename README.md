# chrono-diagnostics

Rust 结构化错误、运行日志和按创建期限滚动的文件日志。

运行 `VERUS=/path/to/verus ./check.sh` 构建并测试此库，执行生产和测试中的全部 Verus 证明。工具版本见 `rust-toolchain.toml` 与检查脚本。原生检查使用 `target/native-checks/` 内的专属目录。成功且读者退出后释放；失败和中断的原件在 24 小时窗口内可转存。主检出的每小时 `harness-maintain-checks` 在窗口到期并取得目录租约后回收。显式设置 `CARGO_TARGET_DIR` 可复用调用方负责保留和回收的 Cargo 缓存。Cargo 使用方可按仓库 URL 和固定提交引用 `chrono-diagnostics`。

源码和测试取自 [ChronoAIProject/chrono-harness 的固定提交](https://github.com/ChronoAIProject/chrono-harness/tree/5f6892dfa2f73675a84e4ebdc3b06eb83b824d6e/crates/diagnostics)。

本仓在 FILEMAP 启用了新增文件分类。新增文件与改动到的未分类逻辑需要登记实际证明根，纯资料登记为 data。未改动的旧逻辑可保留未知。

## 定时回收日志

从本仓 Release 下载当前平台的 `chrono-diagnostics`，按同一 Release 的 `SHA256SUMS` 校验后安装到工作树外。宿主可运行 `chrono-diagnostics expire-logs-v1 ROOT` 回收该检出的到期日志，程序读取现有 `.chrono-harness/logging.json`。没有时间保留策略时，stdout 为 JSON `null`；否则返回 `removed`、`legacy`、`busy` 和 `errors`。成功或写入者正持锁时退出 0，单项删除失败时退出 1 并保留完整 JSON。配置、读取或结果发布失败时退出 2，诊断写入 stderr。

## 记录独立命令的运行日志

启用日志的命令可以调用 `chrono-diagnostics cli-events-v1 ROOT PRODUCER OPERATION PID`。传入实际业务进程的 PID。读取标准输出的一行 `{"ready":true}` 后开始工作。业务可向其标准输入写入 `{"diagnostic":"原始错误"}`，最后写入 `{"exit_code":N}` 并等待子进程结束。每条消息都是单个 JSON 对象。业务的标准输出由业务命令直接交给使用方。日志使用宿主的级别、目录、滚动和过期策略。成功发布日志时返回业务退出码，输入损坏、缺少终态或日志发布失败时返回 2。 同一个会话也接受 `{"event":EVENT}`，用于记录业务进程实际发出的 tracing-tunnel 0.1 事件与 span。原生参数解码失败时可发送 `{"invalid_argument":{"argv_index":N,"bytes":[...]}}`，将原字节及位置发到宿主日志和 stderr，随后报告退出码 2。

## 接收宿主的结构化日志

`chrono-diagnostics trace-events-v1 ROOT PRODUCER` 将调用方的 tracing 事件写入其显式日志流，并采用 ROOT 的日志级别、滚动和过期策略。通信使用 UTF-8 JSON，每行一条。就绪后 stdout 返回 `"ready"`。调用方向 stdin 发送 `{"event":EVENT}`，EVENT 使用 tracing-tunnel 0.1 的格式。每条事件处理完毕返回 `"accepted"`。发送 `"finish"` 后接收 `{"finished":[...]}`，等待进程退出。成功返回空数组和退出码 0，配置、协议或日志失败返回结构化错误记录和退出码 2。

处理事件及结束时，stdout 上的 `{"write":"..."}` 或 `"flush"` 要求调用方写入或刷新它选择的日志流。完成操作后立即回传 `"ok"`，发生错误时回传 `{"error":{"native_code":28,"kind":"StorageFull","custom_source":false,"message":"..."}}`。没有系统错误码时填 null。日志在业务执行期间即可读取。业务程序自行发布原始输出和退出状态。

发布 stderr 上的单次原生错误时，使用 `chrono-diagnostics trace-events-v1 --sink-only PRODUCER`。该模式只向调用方提供的流写入与刷新，按 INFO 级别生成标准事件。传入宿主 ROOT 时仍按宿主政策选择日志目标。

## 开发机检查原件

本仓通过模板选装 `local_check_retention`。保留期限、窗口内转存和定时维护的命令见[本仓工具入口](.chrono-harness/USAGE.md#开发机检查原件)。

GitHub Actions 的单元原件保留 1 天。汇总归档随 GitHub 仓库设置滚动；采用时仓库配置为 90 天。
