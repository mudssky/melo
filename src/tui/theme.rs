/// TUI 主题定义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// 标题前缀。
    pub title_prefix: &'static str,
    /// 次级前缀。
    pub muted_prefix: &'static str,
}

impl Default for Theme {
    /// 返回默认主题配置。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：默认主题
    fn default() -> Self {
        Self {
            title_prefix: "Melo",
            muted_prefix: "Remote",
        }
    }
}
