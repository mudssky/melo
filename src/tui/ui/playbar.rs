use crate::core::model::player::PlayerSnapshot;

/// 生成播放栏文案。
///
/// # 参数
/// - `snapshot`：播放器快照
///
/// # 返回值
/// - `String`：播放栏显示文本
pub fn playback_label(snapshot: &PlayerSnapshot) -> String {
    let title = snapshot
        .current_song
        .as_ref()
        .map(|song| song.title.as_str())
        .unwrap_or("Nothing Playing");
    format!("{} | {}", snapshot.playback_state, title)
}
