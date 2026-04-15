use std::time::{Duration, Instant};

const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(350);

/// 鼠标当前命中的交互目标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseTarget {
    PlaylistRow(usize),
    PreviewRow(usize),
    DetailPanel,
    None,
}

/// 鼠标点击分类结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickKind {
    Single,
    Double,
}

/// 基于时间窗口的软件双击检测器。
#[derive(Debug, Default)]
pub struct ClickTracker {
    last_click: Option<(MouseTarget, Instant)>,
}

impl ClickTracker {
    /// 根据目标和时间戳分类当前点击。
    ///
    /// # 参数
    /// - `target`：本次点击命中的目标
    /// - `now`：本次点击时间
    ///
    /// # 返回值
    /// - `ClickKind`：单击或双击
    pub fn classify(&mut self, target: MouseTarget, now: Instant) -> ClickKind {
        let kind = match self.last_click {
            Some((previous_target, previous_at))
                if previous_target == target
                    && now.duration_since(previous_at) <= DOUBLE_CLICK_WINDOW =>
            {
                ClickKind::Double
            }
            _ => ClickKind::Single,
        };

        self.last_click = Some((target, now));
        kind
    }
}

#[cfg(test)]
mod tests;
