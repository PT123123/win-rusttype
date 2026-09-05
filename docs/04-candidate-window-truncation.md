# 04-candidate-window-truncation

## 问题

终端应用（如 opencode CLI）中，候选窗口底部被终端的输入区域截断。

## 复现步骤

1. 在终端中使用输入法
2. 输入触发候选窗口
3. 候选窗口底部被终端输入区域遮挡/截断

## 原因分析

候选窗口定位逻辑使用 `clamp_point_to_monitor`，只确保窗口在显示器工作区内，不考虑光标下方是否有足够空间。终端应用的光标通常在屏幕底部，候选窗口向下延伸时超出终端可视区域。

## 已尝试的修复

在 `ui.rs` 中添加 `position_relative_to_cursor` 函数，替代所有 `clamp_point_to_monitor` 调用：

```rust
fn position_relative_to_cursor(cx: i32, cy: i32, w: i32, h: i32) -> (i32, i32) {
    let space_below = rc.bottom - cy - gap;
    if space_below >= h {
        cy + gap  // 下方有足够空间，放在下方
    } else {
        cy - h - gap  // 下方空间不足，放在上方
    }
}
```

## 当前状态

修复后问题仍然存在。可能原因：
1. 终端使用 DirectComposition 或 SetWindowRgn 裁剪窗口区域
2. 候选窗口是 WS_POPUP + WS_EX_TOPMOST，但仍被终端渲染层裁剪
3. 需要进一步调查终端窗口的裁剪机制

## 下一步

- 调查终端窗口是否使用 SetWindowRgn 或 DirectComposition 裁剪
- 对比微软输入法在终端中的行为
- 考虑其他定位策略（如使用窗口相对坐标）
