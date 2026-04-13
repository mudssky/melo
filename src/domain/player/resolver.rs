use crate::core::config::settings::PlayerSettings;
use crate::core::error::{MeloError, MeloResult};
use crate::domain::player::factory::BackendChoice;

/// 描述当前运行环境里各播放器后端的可用性。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendAvailability {
    /// `libmpv` 后端是否可用。
    pub mpv_lib: bool,
    /// `mpv-ipc` 后端是否可用。
    pub mpv_ipc: bool,
    /// `rodio` 后端是否可用。
    pub rodio: bool,
}

/// 解析后的具体后端选择结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBackendChoice {
    /// 最终选中的后端。
    pub choice: BackendChoice,
    /// 需要对用户展示的回退提示。
    pub notice: Option<String>,
}

/// 后端解析器，负责统一封装后端优先级和回退规则。
#[derive(Debug, Default, Clone, Copy)]
pub struct BackendResolver;

impl BackendResolver {
    /// 根据配置和可用性探测结果解析最终后端。
    ///
    /// # 参数
    /// - `settings`：播放器配置
    /// - `availability`：当前环境中的后端可用性
    ///
    /// # 返回值
    /// - `MeloResult<ResolvedBackendChoice>`：解析后的后端选择和提示信息
    pub fn resolve_choice(
        &self,
        settings: &PlayerSettings,
        availability: BackendAvailability,
    ) -> MeloResult<ResolvedBackendChoice> {
        match settings.backend.as_str() {
            "rodio" => Ok(ResolvedBackendChoice {
                choice: BackendChoice::Rodio,
                notice: None,
            }),
            "mpv_lib" => {
                if availability.mpv_lib {
                    Ok(ResolvedBackendChoice {
                        choice: BackendChoice::MpvLib,
                        notice: None,
                    })
                } else {
                    Err(MeloError::Message(
                        "mpv_lib_backend_unavailable".to_string(),
                    ))
                }
            }
            "mpv" | "mpv_ipc" => {
                if availability.mpv_ipc {
                    Ok(ResolvedBackendChoice {
                        choice: BackendChoice::MpvIpc,
                        notice: None,
                    })
                } else {
                    Err(MeloError::Message("mpv_backend_unavailable".to_string()))
                }
            }
            _ => {
                if availability.mpv_lib {
                    return Ok(ResolvedBackendChoice {
                        choice: BackendChoice::MpvLib,
                        notice: None,
                    });
                }
                if availability.mpv_ipc {
                    return Ok(ResolvedBackendChoice {
                        choice: BackendChoice::MpvIpc,
                        notice: Some("mpv_lib unavailable, fell back to mpv_ipc".to_string()),
                    });
                }
                if availability.rodio {
                    return Ok(ResolvedBackendChoice {
                        choice: BackendChoice::Rodio,
                        notice: Some(
                            "mpv_lib and mpv_ipc unavailable, fell back to rodio".to_string(),
                        ),
                    });
                }
                Err(MeloError::Message("no_backend_available".to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests;
