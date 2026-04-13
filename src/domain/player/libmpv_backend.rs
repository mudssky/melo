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
    driver: Arc<Mutex<Box<dyn LibmpvDriver>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
}

struct LibmpvPlaybackSession {
    driver: Arc<Mutex<Box<dyn LibmpvDriver>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
}

/// `libmpv` 底层驱动抽象。
trait LibmpvDriver: Send {
    /// 使用 replace 语义装载一个新文件。
    ///
    /// # 参数
    /// - `path`：待播放文件路径
    ///
    /// # 返回值
    /// - `MeloResult<()>`：执行结果
    fn loadfile_replace(&mut self, path: &std::path::Path) -> MeloResult<()>;

    /// 设置当前暂停状态。
    ///
    /// # 参数
    /// - `paused`：目标暂停状态
    ///
    /// # 返回值
    /// - `MeloResult<()>`：执行结果
    fn set_pause(&mut self, paused: bool) -> MeloResult<()>;

    /// 设置当前音量百分比。
    ///
    /// # 参数
    /// - `volume_percent`：目标音量百分比
    ///
    /// # 返回值
    /// - `MeloResult<()>`：执行结果
    fn set_volume(&mut self, volume_percent: f64) -> MeloResult<()>;

    /// 停止当前播放。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<()>`：执行结果
    fn stop(&mut self) -> MeloResult<()>;

    /// 读取当前播放位置。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Option<Duration>`：当前位置
    fn current_position(&mut self) -> Option<Duration>;

    /// 等待一次底层事件，并在需要时返回停止原因。
    ///
    /// # 参数
    /// - `timeout`：等待超时时间
    ///
    /// # 返回值
    /// - `MeloResult<Option<PlaybackStopReason>>`：读取到停止事件时返回停止原因
    fn wait_event(&mut self, timeout: Duration) -> MeloResult<Option<PlaybackStopReason>>;
}

struct RealLibmpvDriver {
    mpv: Mpv,
}

struct NoopLibmpvDriver;

impl LibmpvBackend {
    /// 使用全局配置构造 `libmpv` 后端。
    ///
    /// # 参数
    /// - `settings`：全局配置
    ///
    /// # 返回值
    /// - `MeloResult<Self>`：初始化后的 `libmpv` 后端
    pub fn new(settings: Settings) -> MeloResult<Self> {
        let driver = Box::new(RealLibmpvDriver::new()?) as Box<dyn LibmpvDriver>;
        Ok(Self::with_driver(settings, driver))
    }

    /// 为测试构造不依赖真实 `libmpv` 运行时的后端实例。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Self`：测试用后端实例
    pub fn new_for_test() -> Self {
        Self::with_driver(Settings::default(), Box::new(NoopLibmpvDriver))
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

    /// 使用指定驱动构造后端实例。
    ///
    /// # 参数
    /// - `settings`：全局配置
    /// - `driver`：底层驱动实现
    ///
    /// # 返回值
    /// - `Self`：构造后的后端实例
    fn with_driver(settings: Settings, driver: Box<dyn LibmpvDriver>) -> Self {
        let (runtime_tx, _) = broadcast::channel(16);
        Self {
            _settings: settings,
            driver: Arc::new(Mutex::new(driver)),
            runtime_tx,
        }
    }

    /// 为测试注入自定义驱动实例。
    ///
    /// # 参数
    /// - `driver`：测试驱动
    ///
    /// # 返回值
    /// - `Self`：测试用后端实例
    #[cfg(test)]
    pub(crate) fn new_for_test_with_driver<D>(driver: D) -> Self
    where
        D: LibmpvDriver + 'static,
    {
        Self::with_driver(Settings::default(), Box::new(driver))
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
        {
            let mut driver = self.driver.lock().unwrap();
            driver.set_volume(f64::from(request.volume_factor.max(0.0) * 100.0))?;
            driver.set_pause(false)?;
            driver.loadfile_replace(&request.path)?;
        }

        spawn_event_loop(&self.driver, self.runtime_tx.clone(), request.generation);

        Ok(Box::new(LibmpvPlaybackSession {
            driver: Arc::clone(&self.driver),
            runtime_tx: self.runtime_tx.clone(),
        }))
    }
}

impl PlaybackSessionHandle for LibmpvPlaybackSession {
    fn pause(&self) -> MeloResult<()> {
        self.driver.lock().unwrap().set_pause(true)
    }

    fn resume(&self) -> MeloResult<()> {
        self.driver.lock().unwrap().set_pause(false)
    }

    fn stop(&self) -> MeloResult<()> {
        self.driver.lock().unwrap().stop()
    }

    fn subscribe_runtime_events(&self) -> PlaybackRuntimeReceiver {
        self.runtime_tx.subscribe()
    }

    fn current_position(&self) -> Option<Duration> {
        self.driver.lock().unwrap().current_position()
    }

    fn set_volume(&self, factor: f32) -> MeloResult<()> {
        self.driver
            .lock()
            .unwrap()
            .set_volume(f64::from(factor.max(0.0) * 100.0))
    }
}

impl RealLibmpvDriver {
    /// 构造真实 `libmpv` 驱动。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `MeloResult<Self>`：初始化后的真实驱动
    fn new() -> MeloResult<Self> {
        let mpv = Mpv::with_initializer(|initializer| {
            initializer.set_option("vo", "null")?;
            Ok(())
        })
        .map_err(|err| MeloError::Message(err.to_string()))?;
        Ok(Self { mpv })
    }
}

impl LibmpvDriver for RealLibmpvDriver {
    fn loadfile_replace(&mut self, path: &std::path::Path) -> MeloResult<()> {
        self.mpv
            .command("loadfile", &[path.to_string_lossy().as_ref(), "replace"])
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    fn set_pause(&mut self, paused: bool) -> MeloResult<()> {
        self.mpv
            .set_property("pause", paused)
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    fn set_volume(&mut self, volume_percent: f64) -> MeloResult<()> {
        self.mpv
            .set_property("volume", volume_percent)
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    fn stop(&mut self) -> MeloResult<()> {
        self.mpv
            .command("stop", &[])
            .map_err(|err| MeloError::Message(err.to_string()))
    }

    fn current_position(&mut self) -> Option<Duration> {
        self.mpv
            .get_property::<f64>("time-pos")
            .ok()
            .filter(|seconds| *seconds >= 0.0)
            .map(Duration::from_secs_f64)
    }

    fn wait_event(&mut self, timeout: Duration) -> MeloResult<Option<PlaybackStopReason>> {
        let _ = self.mpv.disable_deprecated_events();
        match self.mpv.wait_event(timeout.as_secs_f64()) {
            Some(Ok(Event::EndFile(reason))) => Ok(Some(match reason {
                mpv_end_file_reason::Eof => PlaybackStopReason::NaturalEof,
                mpv_end_file_reason::Stop => PlaybackStopReason::UserStop,
                mpv_end_file_reason::Quit => PlaybackStopReason::UserClosedBackend,
                _ => PlaybackStopReason::BackendAborted,
            })),
            Some(Ok(Event::Shutdown)) => Ok(Some(PlaybackStopReason::UserClosedBackend)),
            Some(Ok(_)) | None => Ok(None),
            Some(Err(err)) => Err(MeloError::Message(err.to_string())),
        }
    }
}

impl LibmpvDriver for NoopLibmpvDriver {
    fn loadfile_replace(&mut self, _path: &std::path::Path) -> MeloResult<()> {
        Ok(())
    }

    fn set_pause(&mut self, _paused: bool) -> MeloResult<()> {
        Ok(())
    }

    fn set_volume(&mut self, _volume_percent: f64) -> MeloResult<()> {
        Ok(())
    }

    fn stop(&mut self) -> MeloResult<()> {
        Ok(())
    }

    fn current_position(&mut self) -> Option<Duration> {
        None
    }

    fn wait_event(&mut self, _timeout: Duration) -> MeloResult<Option<PlaybackStopReason>> {
        Ok(None)
    }
}

/// 启动后台事件循环，将 `libmpv` 事件映射为统一运行时事件。
///
/// # 参数
/// - `driver`：共享的 `libmpv` 驱动
/// - `runtime_tx`：运行时事件发送器
/// - `generation`：本次播放对应的代次
///
/// # 返回值
/// - 无
fn spawn_event_loop(
    driver: &Arc<Mutex<Box<dyn LibmpvDriver>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
    generation: u64,
) {
    let driver = Arc::clone(driver);
    std::thread::spawn(move || {
        loop {
            let event = {
                let mut driver = driver.lock().unwrap();
                match driver.wait_event(Duration::from_millis(100)) {
                    Ok(Some(reason)) => Some(Ok(PlaybackRuntimeEvent::PlaybackStopped {
                        generation,
                        reason,
                    })),
                    Ok(None) => Some(Err(())),
                    Err(_) => Some(Ok(PlaybackRuntimeEvent::PlaybackStopped {
                        generation,
                        reason: PlaybackStopReason::BackendAborted,
                    })),
                }
            };

            match event {
                Some(Ok(event)) => {
                    let _ = runtime_tx.send(event);
                    break;
                }
                Some(Err(())) => {}
                None => {}
            }
        }
    });
}

#[cfg(test)]
mod tests;
