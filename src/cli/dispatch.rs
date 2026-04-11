use std::ffi::OsString;

/// CLI 顶层原始参数预分发结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dispatch {
    /// 裸 `melo`，走默认启动流程。
    DefaultLaunch,
    /// 把第一个参数视作路径，走 direct-open 流程。
    DirectOpen(String),
    /// 继续交给 Clap 正常解析。
    Clap,
}

/// 在进入 Clap 之前根据原始参数做一次轻量预分发。
///
/// # 参数
/// - `args`：原始参数数组
///
/// # 返回值
/// - `Dispatch`：预分发结果
pub fn dispatch_args(args: &[OsString]) -> Dispatch {
    let Some(first) = args.get(1).and_then(|value| value.to_str()) else {
        return Dispatch::DefaultLaunch;
    };

    if matches!(
        first,
        "play"
            | "pause"
            | "toggle"
            | "next"
            | "prev"
            | "stop"
            | "status"
            | "tui"
            | "daemon"
            | "player"
            | "library"
            | "queue"
            | "playlist"
            | "db"
            | "config"
            | "-h"
            | "--help"
            | "-V"
            | "--version"
    ) {
        return Dispatch::Clap;
    }

    Dispatch::DirectOpen(first.to_string())
}

#[cfg(test)]
mod tests;
