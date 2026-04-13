/// 终端支持的封面显示协议。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverProtocol {
    Kitty,
    Iterm2,
    Sixel,
    Unsupported,
}

/// 基于环境变量探测当前终端支持的图片协议。
///
/// # 参数
/// - `env`：环境变量键值对列表
///
/// # 返回值
/// - `CoverProtocol`：探测出的协议类型
pub fn detect_cover_protocol_from_env(env: &[(String, String)]) -> CoverProtocol {
    let lookup = |key: &str| {
        env.iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value)
    };
    if lookup("TERM").is_some_and(|value| value.contains("kitty")) {
        return CoverProtocol::Kitty;
    }
    if lookup("TERM_PROGRAM").is_some_and(|value| value == "iTerm.app") {
        return CoverProtocol::Iterm2;
    }
    if lookup("TERM").is_some_and(|value| value.contains("sixel")) {
        return CoverProtocol::Sixel;
    }
    CoverProtocol::Unsupported
}

/// 基于协议能力和封面路径生成当前详情区要显示的摘要。
///
/// # 参数
/// - `protocol`：已探测到的终端协议
/// - `artwork_path`：可选封面路径
///
/// # 返回值
/// - `String`：当前可显示的封面摘要
pub fn cover_fallback_summary(protocol: CoverProtocol, artwork_path: Option<&str>) -> String {
    match (protocol, artwork_path) {
        (CoverProtocol::Unsupported, Some(_)) => "Cover unsupported in this terminal".to_string(),
        (_, Some(path)) => format!("Cover: {path}"),
        (_, None) => "No cover available".to_string(),
    }
}

#[cfg(test)]
mod tests;
