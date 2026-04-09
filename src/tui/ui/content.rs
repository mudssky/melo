use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// 按显示宽度裁剪歌曲标题，避免 CJK 宽字符撑破列表列宽。
///
/// # 参数
/// - `title`：原始标题
/// - `max_width`：最大显示宽度
///
/// # 返回值
/// - `String`：适配列宽后的标题
pub fn render_song_title(title: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    if UnicodeWidthStr::width(title) <= max_width {
        return title.to_string();
    }

    if max_width == 1 {
        return "…".to_string();
    }

    let mut rendered = String::new();
    let mut width = 0;
    for ch in title.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > max_width - 1 {
            break;
        }
        rendered.push(ch);
        width += ch_width;
    }
    rendered.push('…');
    rendered
}
