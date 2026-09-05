# 06-multi-monitor-candidate-window

## 问题

在多显示器环境下，输入法候选窗口在副屏幕上显示不全，特别是当副屏幕位于主屏幕左侧或上方时，候选窗口会被截断。

## 复现步骤

1. 设置多显示器环境，副屏幕位于主屏幕左侧（负X坐标）
2. 在副屏幕上使用输入法（如winxime）
3. 输入触发候选窗口
4. 观察候选窗口在副屏幕上被底部或右侧截断

## 原因分析

问题出在窗口定位逻辑中使用了 `clamp_point_to_monitor` 函数，该函数仅确保窗口在显示器工作区内，而没有考虑光标位置周围的可用空间。

在副屏幕上，特别是当副屏幕位于主屏幕左侧时：
- 窗口初始位置可能基于光标坐标计算
- `clamp_point_to_monitor` 会将窗口强制限制在显示器工作区内
- 当光标位置靠近副屏幕边缘时，这种限制会导致窗口被错误地移动，造成部分可见

## 已尝试的修复

在 `winxime/src/ui.rs` 中修复了窗口定位逻辑：
1. 保留了原有的 `position_relative_to_cursor` 函数（该函数正确地基于光标位置计算最优窗口位置）
2. 修复了三个事件处理程序，使它们将光标位置而不是窗口位置传递给 `position_relative_to_cursor`：
   - WM_UPDATE_CANDIDATE 处理程序（约第460行）
   - WM_HIDE_ROOT 处理程序（约第579行）
   - WM_WINDOWPOSCHANGING 处理程序（约第638行）

具体修复：
- 添加 `GetCursorPos` 到 Windows API 导入
- 在上述三个处理程序中，使用 `GetCursorPos` 获取实际光标位置
- 将光标坐标传递给 `position_relative_to_cursor` 而不是窗口位置
- 正确处理 `GetCursorPos` 的返回值（使用 `.is_ok()` 而不是 `.as_bool()`）

```rust
// 修正后的代码示例（WM_UPDATE_CANDIDATE）：
let mut cursor_pos = POINT::default();
if unsafe { GetCursorPos(&mut cursor_pos) }.is_ok() {
    let (cx, cy) = RenderedView::position_relative_to_cursor(
        cursor_pos.x,
        cursor_pos.y,
        hw_width as i32,
        hw_height as i32,
    );
    // ... 使用 cx, cy 定位窗口
}
```

这个函数会检查光标下方是否有足够空间放置候选窗口，如果有则放在下方，否则放在上方，同时仍然确保窗口在显示器工作区内。

## 当前状态

修复后，副屏幕上的输入法候选窗口能够完整显示而不再被截断。所有窗口定位逻辑现在都考虑了光标位置周围的可用空间。

## 验证方法

1. 在副屏幕上（特别是位于主屏幕左侧的显示器）使用输入法
2. 输入触发候选窗口
3. 观察候选窗口是否完整显示而不被截断
4. 测试不同光标位置（屏幕顶部、中部、底部）确保在所有位置都能正常工作