use serde::{Deserialize, Serialize};

use crate::core::error::{MeloError, MeloResult};
use crate::core::model::player::RepeatMode;

/// 面向用户的播放模式枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackMode {
    /// 顺序播放。
    Ordered,
    /// 单曲循环。
    RepeatOne,
    /// 随机播放。
    Shuffle,
    /// 单曲播放后停止。
    Single,
}

/// 将用户播放模式投影到现有 `repeat + shuffle` 语义后的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackModeProjection {
    /// 兼容旧播放器状态的循环模式。
    pub repeat_mode: RepeatMode,
    /// 是否启用随机播放。
    pub shuffle_enabled: bool,
    /// 是否在当前曲播放结束后停止。
    pub stop_after_current: bool,
}

impl PlaybackMode {
    /// 返回当前播放模式对应的稳定字符串。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `&'static str`：稳定播放模式文本
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ordered => "ordered",
            Self::RepeatOne => "repeat_one",
            Self::Shuffle => "shuffle",
            Self::Single => "single",
        }
    }

    /// 从配置字符串解析播放模式。
    ///
    /// # 参数
    /// - `value`：配置中的播放模式文本
    ///
    /// # 返回值
    /// - `MeloResult<Self>`：解析成功时返回对应播放模式
    pub fn from_config(value: &str) -> MeloResult<Self> {
        match value {
            "ordered" => Ok(Self::Ordered),
            "repeat_one" => Ok(Self::RepeatOne),
            "shuffle" => Ok(Self::Shuffle),
            "single" => Ok(Self::Single),
            other => Err(MeloError::Message(format!("invalid_playback_mode:{other}"))),
        }
    }

    /// 将用户播放模式投影为旧播放器状态字段组合。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `PlaybackModeProjection`：兼容旧字段的投影结果
    pub fn project(self) -> PlaybackModeProjection {
        match self {
            Self::Ordered => PlaybackModeProjection {
                repeat_mode: RepeatMode::Off,
                shuffle_enabled: false,
                stop_after_current: false,
            },
            Self::RepeatOne => PlaybackModeProjection {
                repeat_mode: RepeatMode::One,
                shuffle_enabled: false,
                stop_after_current: false,
            },
            Self::Shuffle => PlaybackModeProjection {
                repeat_mode: RepeatMode::Off,
                shuffle_enabled: true,
                stop_after_current: false,
            },
            Self::Single => PlaybackModeProjection {
                repeat_mode: RepeatMode::Off,
                shuffle_enabled: false,
                stop_after_current: true,
            },
        }
    }
}

#[cfg(test)]
mod tests;
