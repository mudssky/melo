use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::domain::player::backend::PlaybackBackend;
use crate::domain::player::backend::PlaybackStartRequest;
use crate::domain::player::runtime::PlaybackStopReason;

#[test]
fn libmpv_end_file_reason_maps_to_runtime_reason() {
    assert_eq!(
        super::map_end_file_reason("eof"),
        PlaybackStopReason::NaturalEof
    );
    assert_eq!(
        super::map_end_file_reason("stop"),
        PlaybackStopReason::UserStop
    );
    assert_eq!(
        super::map_end_file_reason("quit"),
        PlaybackStopReason::UserClosedBackend
    );
}

#[test]
fn libmpv_backend_reports_stable_name() {
    let backend = super::LibmpvBackend::new_for_test();
    assert_eq!(backend.backend_name(), "mpv_lib");
}

#[test]
fn libmpv_backend_reuses_driver_for_sequential_loads() {
    let driver = super::tests::FakeLibmpvDriver::default();
    let backend = super::LibmpvBackend::new_for_test_with_driver(driver.clone());

    backend.start_session(play_request("A.flac", 1)).unwrap();
    backend.start_session(play_request("B.flac", 2)).unwrap();

    assert_eq!(driver.created_instances(), 1);
    assert_eq!(driver.loadfile_calls(), vec!["A.flac", "B.flac"]);
}

#[test]
fn libmpv_backend_stop_keeps_driver_alive_for_future_reload() {
    let driver = super::tests::FakeLibmpvDriver::default();
    let backend = super::LibmpvBackend::new_for_test_with_driver(driver.clone());

    let session = backend.start_session(play_request("A.flac", 1)).unwrap();
    session.stop().unwrap();
    backend.start_session(play_request("B.flac", 2)).unwrap();

    assert_eq!(driver.created_instances(), 1);
}

/// 构造测试用播放启动请求。
///
/// # 参数
/// - `path`：测试音频路径
/// - `generation`：播放代次
///
/// # 返回值
/// - `PlaybackStartRequest`：测试用播放请求
fn play_request(path: &str, generation: u64) -> PlaybackStartRequest {
    PlaybackStartRequest {
        path: std::path::PathBuf::from(path),
        generation,
        volume_factor: 1.0,
    }
}

#[derive(Clone)]
pub(super) struct FakeLibmpvDriver {
    state: Arc<Mutex<FakeLibmpvDriverState>>,
}

#[derive(Default)]
struct FakeLibmpvDriverState {
    created_instances: usize,
    loadfile_calls: Vec<String>,
}

impl Default for FakeLibmpvDriver {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeLibmpvDriverState {
                created_instances: 1,
                loadfile_calls: Vec::new(),
            })),
        }
    }
}

impl FakeLibmpvDriver {
    /// 返回驱动实例创建次数。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `usize`：创建次数
    fn created_instances(&self) -> usize {
        self.state.lock().unwrap().created_instances
    }

    /// 返回所有 `loadfile replace` 调用记录。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `Vec<String>`：加载过的路径列表
    fn loadfile_calls(&self) -> Vec<String> {
        self.state.lock().unwrap().loadfile_calls.clone()
    }
}

impl super::LibmpvDriver for FakeLibmpvDriver {
    fn loadfile_replace(&mut self, path: &std::path::Path) -> crate::core::error::MeloResult<()> {
        self.state
            .lock()
            .unwrap()
            .loadfile_calls
            .push(path.to_string_lossy().to_string());
        Ok(())
    }

    fn set_pause(&mut self, _paused: bool) -> crate::core::error::MeloResult<()> {
        Ok(())
    }

    fn set_volume(&mut self, _volume_percent: f64) -> crate::core::error::MeloResult<()> {
        Ok(())
    }

    fn stop(&mut self) -> crate::core::error::MeloResult<()> {
        Ok(())
    }

    fn current_position(&mut self) -> Option<Duration> {
        None
    }

    fn wait_event(
        &mut self,
        _timeout: Duration,
    ) -> crate::core::error::MeloResult<Option<PlaybackStopReason>> {
        Ok(None)
    }
}
