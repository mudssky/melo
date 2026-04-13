/// 通用列表视口状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportState {
    /// 当前可见高度。
    pub visible_height: usize,
    /// 当前滚动起点。
    pub scroll_top: usize,
}

impl ViewportState {
    /// 创建新的视口状态。
    ///
    /// # 参数
    /// - `visible_height`：当前可见高度
    ///
    /// # 返回值
    /// - `Self`：初始化后的视口状态
    pub fn new(visible_height: usize) -> Self {
        Self {
            visible_height,
            scroll_top: 0,
        }
    }

    /// 让视口跟随当前选中项，确保选中项留在可见窗口中。
    ///
    /// # 参数
    /// - `selected_index`：当前选中索引
    /// - `item_count`：列表总长度
    ///
    /// # 返回值
    /// - 无
    pub fn follow_selection(&mut self, selected_index: usize, item_count: usize) {
        if selected_index < self.scroll_top {
            self.scroll_top = selected_index;
        } else if selected_index >= self.scroll_top.saturating_add(self.visible_height) {
            self.scroll_top = selected_index + 1 - self.visible_height;
        }

        self.scroll_top = self
            .scroll_top
            .min(item_count.saturating_sub(self.visible_height));
    }
}

#[cfg(test)]
mod tests;
