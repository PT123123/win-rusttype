# Docs 索引

本项目（凑合输入法，winxime fork + librime 引擎）的技术文档。

| 文件 | 内容 |
|------|------|
| [01-gaps.md](01-gaps.md) | 差距分析：MVP 已交付 vs 可用输入法还缺什么（功能/工程/技术债/量化对比） |
| [02-optimization.md](02-optimization.md) | 优化路线图：P0 性能 → P1 界面精致度 → P2 功能 → P3 工程化，含验收标准与竞品对标方法学 |
| [03-settings-menu-fix.md](03-settings-menu-fix.md) | 排查记录：托盘「设置/关于」无反应（安装清单漏文件 + UAC Installer Detection 致 os error 740），含修复与通用经验 |
| [05-server-crash-analysis.md](05-server-crash-analysis.md) | 排查记录：server 运行中 crash 但无 panic/abort 日志的定位分析 |
| [06-multi-monitor-candidate-window.md](06-multi-monitor-candidate-window.md) | 副屏幕候选窗口显示不全问题：多显示器环境下输入法候选窗口被截断的原因分析及修复 |
| [FINAL_SUMMARY.md](FINAL_SUMMARY.md) | 多显示器候选窗截断的早期排查报告（中文存档；其结论已被 winxime-server/ui.rs 的最终修复取代） |
| [FIX_SUMMARY.md](FIX_SUMMARY.md) | 多显示器候选窗截断的早期排查报告（英文存档；其结论已被 winxime-server/ui.rs 的最终修复取代） |

## 关键事实速查

- **工作根**：本仓库根目录（含 `winxime\`、`libximecore\`、`docs\`）
- **构建产物**：`winxime\target\debug\`（winxime_tsf.dll / winxime-server.exe / winxime-setup.exe / rime.dll）
- **方案数据**：`winxime\rime-wubi\`（默认 pinyin_simp，已改 schema_list 两处：default.yaml + default.custom.yaml）
- **用户数据**：`winxime\target\debug\user-data\`（server 启动时自动拷贝 + deploy）
- **基准工具**：`winxime\target\debug\perf3.exe`（逐键延迟）、`perf.exe`、`perf2.exe`、`ipc-test.exe`
- **安装脚本**：`winxime\install.ps1`（复制产物 → 提权注册 → 启动 server → 开机自启）
- **基线数据**：server Private 25.2MB / 逐键 p50 2.3-2.9ms（debug 构建，2026-09-05 测）
- **架构**：TSF DLL（winxime_tsf.dll）↔ named pipe `\\.\pipe\WinximeNamedPipe` ↔ server（RIME 引擎 + Direct2D 候选窗 + 托盘）
