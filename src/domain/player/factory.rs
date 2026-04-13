use std::sync::Arc;

use crate::core::config::settings::{PlayerSettings, Settings};
use crate::core::error::{MeloError, MeloResult};
use crate::domain::player::backend::PlaybackBackend;
use crate::domain::player::mpv_backend::{MpvBackend, mpv_exists};
use crate::domain::player::rodio_backend::RodioBackend;

/// 配置解析后的具体后端选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendChoice {
    /// 使用 `rodio` 后端。
    Rodio,
    /// 使用 `mpv-ipc` 后端。
    MpvIpc,
    /// 使用 `libmpv` 后端。
    MpvLib,
}

/// 根据配置和环境探测结果解析后端选择。
///
/// # 参数
/// - `settings`：播放器配置
/// - `mpv_available`：用于探测 `mpv` 是否可用的函数
///
/// # 返回值
/// - `MeloResult<BackendChoice>`：解析后的具体后端
pub fn resolve_backend_choice(
    settings: &PlayerSettings,
    mpv_available: impl Fn() -> bool,
) -> MeloResult<BackendChoice> {
    match settings.backend.as_str() {
        "rodio" => Ok(BackendChoice::Rodio),
        "mpv_lib" => Ok(BackendChoice::MpvLib),
        "mpv" | "mpv_ipc" => {
            if mpv_available() {
                Ok(BackendChoice::MpvIpc)
            } else {
                Err(MeloError::Message("mpv_backend_unavailable".to_string()))
            }
        }
        _ => {
            if mpv_available() {
                Ok(BackendChoice::MpvIpc)
            } else {
                Ok(BackendChoice::Rodio)
            }
        }
    }
}

/// 根据显式后端选择构造具体播放后端。
///
/// # 参数
/// - `choice`：已经解析完成的后端选择
/// - `settings`：全局配置
///
/// # 返回值
/// - `MeloResult<Arc<dyn PlaybackBackend>>`：构造出的播放后端实例
pub fn build_backend_for_choice(
    choice: BackendChoice,
    settings: &Settings,
) -> MeloResult<Arc<dyn PlaybackBackend>> {
    match choice {
        BackendChoice::Rodio => Ok(Arc::new(RodioBackend::new()?)),
        BackendChoice::MpvIpc => Ok(Arc::new(MpvBackend::new(settings.clone())?)),
        BackendChoice::MpvLib => Err(MeloError::Message(
            "mpv_lib_backend_unavailable".to_string(),
        )),
    }
}

/// 根据当前配置构造具体播放后端。
///
/// # 参数
/// - `settings`：全局配置
///
/// # 返回值
/// - `MeloResult<Arc<dyn PlaybackBackend>>`：可用的播放后端实例
pub fn build_backend(settings: &Settings) -> MeloResult<Arc<dyn PlaybackBackend>> {
    let choice =
        resolve_backend_choice(&settings.player, || mpv_exists(&settings.player.mpv.path))?;
    build_backend_for_choice(choice, settings)
}

#[cfg(test)]
mod tests;
