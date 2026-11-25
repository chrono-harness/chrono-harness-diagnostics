# chrono-harness-diagnostics 使用说明

组件的用途、开发仓库和示例见 [Chrono Harness 用户与开发者入口](https://github.com/chrono-harness/chrono-harness-main)。

Rust 结构化错误、运行日志和按创建期限滚动的文件日志。

开发分支为 `dev`。日常 PR 合入开发分支，正式版从 `main` 发布。

首次检出运行 `mise trust` 和 `mise run harness-setup`。工具版本见 [.config/mise/conf.d/chrono-harness.toml](../.config/mise/conf.d/chrono-harness.toml)。

开始任务时运行 `mise run harness-setup` 刷新根指南，成功后重新阅读本仓的 `AGENTS.md` 或 `CLAUDE.md`。完整规则和本仓开发步骤见根 [CLAUDE.md](../CLAUDE.md)。

用 `mise exec -- chrono-pr pr-reminders` 查看本地提醒，再运行 `mise exec -- chrono-pr pr-status --pr NUMBER --head-sha SHA` 查询 GitHub。

## 本仓验证

产品位于 [`crates/diagnostics`](../crates/diagnostics)。原生测试位于 [`crates/diagnostics-tests`](../crates/diagnostics-tests)。在仓库根目录运行 `mise exec -- ./check.sh` 执行本仓的检查。 本地和 GitHub 使用同一份 Git DELTA、证明根与原生测试登记。单元收集六项 Verus 验证和原生测试回执，汇总作业读取原件。

## 开发机检查原件

本地检查使用 runner 登记的原件供单元报告、汇总和排错读取。保留期限由本仓 `.chrono-harness/retention.json` 决定，本仓已提交 24 小时策略，开发机首次安装沿用现有文件。需要长期使用的报告和原件应在窗口内复制到持久存储。维护回收已经到期且引用已释放的原件。当前报告和活跃读者能继续引用原件；最后一次报告一直保留时，这部分文件也可能一直存在。启用前生成的未登记文件由原生产者负责。

运行 `mise run harness-maintain-checks` 进行一次维护。长期检出可在本机用户的 crontab 中加入下列条目，将仓库和 mise 路径换为本机绝对路径。定时器每小时唤起同一入口，启用后用原件的实际消失核对回收。在删除条目之前应保全有价值的结果，并完成剩余清理。

```cron
0 * * * * cd '/absolute/path/to/repository' && '/absolute/path/to/mise' run harness-maintain-checks >/dev/null
```
