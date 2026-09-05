# 凑合输入法 — 优化路线图

> 原则：**性能是硬指标，界面精致度是核心卖点**（用户立项目标：超越搜狗/微信）。
> 所有优化必须有可量化验收标准（基准工具 `winxime\target\debug\perf3.exe` 已就绪）。
> 优先级：P0 = 必须做，P1 = 近期做，P2 = 中期，P3 = 远期。

---

## P0 — 性能与基础（2-3 天）

### 0.1 release 构建 + 重新测基线
- **动作**：`cargo build --release`（需确认 librime-sys2 的 cmake 在 release 下正确出优化二进制）
- **验收**：perf3 逐键 p50 < 1.5ms；server Private < 15MB
- **风险**：wgpu/iced 的 release 构建时间长；librime cmake 构建类型需验证（CMAKE_BUILD_TYPE=Release）

### 0.2 日志降级（立即见效的免费性能）
- **现状**：`info!` 级逐键输出（ipc_server.rs 每键 5+ 行、ui.rs on_paint 每帧 10+ 行）
- **动作**：
  1. 生产默认 `RUST_LOG=warn`
  2. 关键路径的 info! 降为 debug!
  3. 日志文件轮转（大小/日期切割，避免长跑膨胀）
- **验收**：RUST_LOG=warn 下 perf3 不因日志 IO 抖动

### 0.3 Escape 后候选窗重建延迟（8ms → 目标 <2ms）
- **根因**：Esc 清空 → `WM_HIDE_CANDIDATE` 销毁/隐藏窗口 → 下键重建 Direct2D 目标
- **动作**：候选窗**延迟销毁**（隐藏后保留 RenderedView 资源 N 秒）；或首次创建后缓存复用
- **验收**：perf3 增加"Esc 后首键"场景，p50 < 3ms

### 0.4 候选窗增量渲染
- **现状**：每键 ResizeBuffers + 全量重绘全部候选
- **动作**：仅当候选内容/选中项变化时重绘对应区域；ResizeBuffers 仅在尺寸变化时调用
- **验收**：perf3 UI 路径（候选渲染线程）与 IPC 路径解耦，逐键延迟不含渲染帧等待

### 0.5 首启 deploy 体验
- **现状**：首启 deploy 0.4s（仅 pinyin_simp）；方案数据增多后会变慢
- **动作**：deploy 进度回调 → 托盘提示；增量 deploy（RIME 已支持，确认开启）

## P1 — 界面精致度（对标微信输入法，3-5 天）

### 1.1 毛玻璃/Acrylic 背景
- **动作**：`SetWindowCompositionAttribute`（ACCENT_ENABLE_ACRYLICBLURBEHIND）替代当前纯色 + 假阴影
- **效果**：候选窗透出桌面内容，微信输入法同款质感
- **注意**：DComp + Acrylic 组合的兼容性需实测；fallback 保留现有高斯模糊阴影

### 1.2 微动画
- **动作**：
  - 候选窗淡入/淡出（150-200ms，DComp 的 opacity 动画）
  - 选中高亮滑动过渡（当前瞬时跳变）
  - 翻页时候选内容轻扫
- **注意**：动画不得阻塞输入事件处理（异步/独立线程）

### 1.3 多主题系统
- **动作**：配置化主题（背景/文字/高亮/边框/圆角/阴影），预置 3 套：
  - 浅色（当前，提亮打磨）
  - 深色（跟随系统 `AppsUseLightTheme`）
  - 跟随主色（primary_color 驱动，已有雏形）
- **验收**：设置页可切换，重启生效，深浅色跟随系统即时切换

### 1.4 候选窗交互补全
- **动作**：
  - 鼠标悬停高亮 + 点击选词（WM_MOUSEMOVE/HITTEST）
  - 滚轮翻页（WM_MOUSEWHEEL）
  - 页码区域点击（上/下页箭头）
- **验收**：纯鼠标可完成 输入→翻页→选词 全流程

### 1.5 托盘菜单完善
- **动作**：设置、方案切换（子菜单列出 schema_list）、中英切换、退出（确认对话框）、关于
- **现状**：已有 输入法设置/关于/反馈/退出，方案切换缺失

### 1.6 候选窗细节打磨
- **动作**：
  - 选中项圆角胶囊化（radius = item_height/2）或保持圆角矩形 + 更细腻阴影
  - 页码区与候选区视觉分隔（细分隔线）
  - 候选间 hover 态、selkey 弱化（微信风格：选中项 selkey 淡化、文字加粗）
  - 多显示器 DPI 感知验证（GetDpiForMonitor 已有，需测跨屏拖动）

## P2 — 功能（1-2 周）

| # | 项 | 动作 | 验收 |
|---|----|------|------|
| 1 | 中英切换完善 | ascii_composer 配置化（Shift/Caps 可选）；设置页可配 | 设置切换键后即时生效 |
| 2 | 模糊音 | pinyin_simp custom 加 fuzzy（n/l、zh/z、in/ing、an/ang 等） | 输入 `shenme` 出「什么」 |
| 3 | 方案市场 | 已有 IPC：FetchSchemaIndex/DownloadSchema/InstallSchema；补 UI 入口 | 设置页在线装方案 |
| 4 | 简繁切换 | 集成 opencc（librime 已链接 opencc）配置简繁过滤器 | 快捷键切换简繁 |
| 5 | 符号快捷 | 配置 symbols.yaml 触发键（`/` 前缀） | 输入 `/rq` 出日期 |
| 6 | 用户词典管理 | 暴露 user.txt 路径；设置页导入/导出 | 词库可备份迁移 |

## P3 — 工程化（持续）

### 3.1 CI/CD
- **动作**：GitHub Actions（windows-latest）+ swatinem/rust-cache + CMake 3.30 固定版本
- **流程**：push → cargo fmt/clippy → build debug+release → 跑 perf3 断言基线 → 打包 artifact
- **验收**：任何 PR 自动产出可安装 zip

### 3.2 安装器（MSI/WiX 或 Inno Setup）
- **动作**：包 4 exe/dll + 数据目录 + HKLM 注册（DllRegisterServer/Profile/InstallLayoutOrTip）+ 卸载清理
- **验收**：双击安装 → 立即可用；卸载后系统无残留注册表

### 3.3 代码签名
- **动作**：EV 证书（约 ¥2000/年）或 OV 证书；补 SmartScreen 信誉
- **验收**：全新机器下载安装无警告

### 3.4 自动化测试
- **动作**：
  - 单元：ipc 协议编解码、keysym 映射、配置解析
  - 集成：TSF 冒烟（UI Automation 启动记事本 → 切换输入法 → 注入按键 → 断言上屏文本）
- **验收**：CI 中 TSF 冒烟可跑（需自托管 runner 或 Windows VM）

### 3.5 崩溃可观测
- **动作**：WER LocalDumps 注册表指向崩溃目录；server 内 panic hook 写 minidump（windows crate 的 MiniDumpWriteDump）
- **验收**：人为 panic 能产出可分析 dump

### 3.6 性能回归门禁
- **动作**：perf3 输出 JSON → CI 对比上次基线，超阈值 fail
- **阈值建议**：逐键 p50 退化 >20% 即 fail

## 性能对标方法学（如何"证明"超越竞品）

1. **内存**：同机安装搜狗/微信输入法 → 常驻态采样 WorkingSet/Private/句柄（Process Explorer 或 PowerShell 定时采样），记录**空闲态**与**打字态**两组
2. **延迟**：键盘→屏幕上屏的端到端延迟用高速相机（240fps+）或 ETW（PerfView，键盘 ISR → TSF → 应用 WM_CHAR）取证
3. **本工具基准**：perf3 给出 IPC+RIME 处理延迟（不含 TSF 层），文档注明口径，与竞品端到端数值对比时说明差异
4. **复现脚本**：基准结果写 `docs/benchmarks/`，附采样命令与原始数据，保证可复核

## 关键依赖风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| Acrylic 与 DComp swapchain 冲突 | 毛玻璃做不出来 | 先 PoC 验证；失败则用高斯模糊阴影+高透明度近似 |
| wgpu/iced 设置页 release 体积大（55MB debug） | 安装包过大 | 设置页可换 egui/原生 Win32；或接受体积换取开发效率 |
| librime-sys2 release 下 cmake 构建类型 | 性能不达标 | 验证 CMAKE_BUILD_TYPE 传递；必要时 patch build.rs |
| TSF 注册在非管理员环境 | 安装失败 | 安装器必须提权（已有 HKLM 流程）；文档注明 |
