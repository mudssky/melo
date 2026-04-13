use std::sync::{Arc, Mutex};
use std::time::Duration;

use libmpv2::events::Event;
use libmpv2::{Mpv, mpv_end_file_reason};
use tokio::sync::broadcast;

use crate::core::config::settings::Settings;
use crate::core::error::{MeloError, MeloResult};
use crate::domain::player::backend::{
    PlaybackBackend, PlaybackSessionHandle, PlaybackStartRequest,
};
use crate::domain::player::runtime::{
    PlaybackRuntimeEvent, PlaybackRuntimeReceiver, PlaybackStopReason,
};

/// 基于 `libmpv` 的播放后端。
pub struct LibmpvBackend {
    _settings: Settings,
}

struct LibmpvPlaybackSession {
    mpv: Arc<Mutex<Mpv>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
}

impl LibmpvBackend {
    /// 使用全局配置构造 `libmpv` 后端。
    ///
    /// # 参数
    /// - `settings`：全局配置
    ///
    /// # 返回值
    /// - `MeloResult<Self>`：初始化后的 `libmpv` 后端
    pub fn new(settings: Settings) -> MeloResult<Self> {
        Ok(Self {
            _settings: settings,
        })
    }

    /// 为测试构造不依赖真实 `libmpv` 运行时的后端实例。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：测试用后端实例
    pub fn new_for_test() -> Self {
        Self {
            _settings: Settings::default(),
        }
    }

    /// 探测当前环境是否可成功初始化 `libmpv`。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `bool`：是否可用
    pub fn is_available() -> bool {
        Mpv::new().is_ok()
    }
}

/// 将 `libmpv` 的结束原因文本映射为播放器运行时停止原因。
///
/// # 参数
/// - `reason`：`libmpv` 事件里提供的结束原因文本
///
/// # 返回值
/// - `PlaybackStopReason`：统一后的停止原因
pub fn map_end_file_reason(reason: &str) -> PlaybackStopReason {
    match reason {
        "eof" => PlaybackStopReason::NaturalEof,
        "stop" => PlaybackStopReason::UserStop,
        "quit" => PlaybackStopReason::UserClosedBackend,
        _ => PlaybackStopReason::BackendAborted,
    }
}

/// 探测当前环境里是否存在可用的 `libmpv` 运行时。
///
/// # 参数
/// - 无
///
/// # 返回值
/// - `bool`：是否可用
pub fn libmpv_available() -> bool {
    LibmpvBackend::is_available()
}

impl PlaybackBackend for LibmpvBackend {
    fn backend_name(&self) -> &'static str {
        "mpv_lib"
    }

    fn start_session(
        &self,
        request: PlaybackStartRequest,
    ) -> MeloResult<Box<dyn PlaybackSessionHandle>> {
        let mpv = Mpv::with_initializer(|initializer| {
            initializer.set_option("vo", "null")?;
            Ok(())
        })
        .map_err(|err| MeloError::Message(err.to_string()))?;
        let mpv = Arc::new(Mutex::new(mpv));
        {
            let mpv = mpv.lock().unwrap();
            mpv.set_property("pause", false)
                .map_err(|err| MeloError::Message(err.to_string()))?;
            mpv.set_property("volume", f64::from(request.volume_factor.max(0.0) * 100.0))
                .map_err(|err| MeloError::Message(err.to_string()))?;
            mpv.command(
                "loadfile",
                &[request.path.to_string_lossy().as_ref(), "replace"],
            )
            .map_err(|err| MeloError::Message(err.to_string()))?;
        }

        let (runtime_tx, _) = broadcast::channel(16);
        spawn_event_loop(&mpv, runtime_tx.clone(), request.generation);

        Ok(Box::new(LibmpvPlaybackSession { mpv, runtime_tx }))
    }
}

impl PlaybackSessionHandle for LibmpvPlaybackSession {
    fn pause(&self) -> MeloResult<()> {
        self.mpv
            .lock()
            .unwrap()
            .set_property("pause", true)
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    fn resume(&self) -> MeloResult<()> {
        self.mpv
            .lock()
            .unwrap()
            .set_property("pause", false)
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    fn stop(&self) -> MeloResult<()> {
        self.mpv
            .lock()
            .unwrap()
            .command("stop", &[])
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    fn subscribe_runtime_events(&self) -> PlaybackRuntimeReceiver {
        self.runtime_tx.subscribe()
    }

    fn current_position(&self) -> Option<Duration> {
        self.mpv
            .lock()
            .unwrap()
            .get_property::<f64>("time-pos")
            .ok()
            .filter(|seconds| *seconds >= 0.0)
            .map(Duration::from_secs_f64)
    }

    fn set_volume(&self, factor: f32) -> MeloResult<()> {
        self.mpv
            .lock()
            .unwrap()
            .set_property("volume", f64::from(factor.max(0.0) * 100.0))
            .map_err(|err| MeloError::Message(err.to_string()))
    }
}

/// 启动后台事件循环，将 `libmpv` 事件映射为统一运行时事件。
///
/// # 参数
/// - `mpv`：共享的 `libmpv` 实例
/// - `runtime_tx`：运行时事件发送器
/// - `generation`：本次播放对应的代次
///
/// # 返回值
/// - 无
fn spawn_event_loop(
    mpv: &Arc<Mutex<Mpv>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
    generation: u64,
) {
    let client_name = format!("melo-libmpv-{generation}");
    let Ok(mut events) = mpv
        .lock()
        .unwrap()
        .create_client(Some(client_name.as_str()))
        .map_err(|err| MeloError::Message(err.to_string()))
    else {
        return;
    };
    std::thread::spawn(move || {
        let _ = events.disable_deprecated_events();
        loop {
            match events.wait_event(0.1) {
                Some(Ok(Event::EndFile(reason))) => {
                    let reason = match reason {
                        mpv_end_file_reason::Eof => PlaybackStopReason::NaturalEof,
                        mpv_end_file_reason::Stop => PlaybackStopReason::UserStop,
                        mpv_end_file_reason::Quit => PlaybackStopReason::UserClosedBackend,
                        _ => PlaybackStopReason::BackendAborted,
                    };
                    let _ = runtime_tx
                        .send(PlaybackRuntimeEvent::PlaybackStopped { generation, reason });
                    break;
                }
                Some(Ok(Event::Shutdown)) => {
                    let _ = runtime_tx.send(PlaybackRuntimeEvent::PlaybackStopped {
                        generation,
                        reason: PlaybackStopReason::UserClosedBackend,
                    });
                    break;
                }
                Some(Ok(_)) => {}
                Some(Err(_)) => break,
                None => {}
            }
        }
    });
}

#[cfg(test)]
mod tests;
