use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::broadcast;

use crate::core::config::settings::Settings;
use crate::core::error::{MeloError, MeloResult};
use crate::domain::player::backend::{
    PlaybackBackend, PlaybackSessionHandle, PlaybackStartRequest,
};
use crate::domain::player::runtime::{
    PlaybackRuntimeEvent, PlaybackRuntimeReceiver, PlaybackStopReason,
};

const IPC_CONNECT_ATTEMPTS: usize = 40;
const IPC_CONNECT_DELAY_MS: u64 = 50;

struct MpvProcess {
    child: Child,
    command_pipe: File,
    ipc_path: String,
    generation: u64,
}

/// 基于外部 `mpv` 进程的播放后端。
pub struct MpvBackend {
    mpv_path: String,
    ipc_dir: String,
    extra_args: Vec<String>,
    process: Arc<Mutex<Option<MpvProcess>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
    current_position: Arc<Mutex<Option<Duration>>>,
    expected_stop_generation: Arc<AtomicU64>,
}

struct MpvPlaybackSession {
    process: Arc<Mutex<Option<MpvProcess>>>,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
    current_position: Arc<Mutex<Option<Duration>>>,
    expected_stop_generation: Arc<AtomicU64>,
}

impl MpvBackend {
    /// 使用全局配置构造一个 `mpv` 播放后端。
    ///
    /// # 参数
    /// - `settings`：全局配置
    ///
    /// # 返回值
    /// - `MeloResult<Self>`：初始化后的 `mpv` 后端
    pub fn new(settings: Settings) -> MeloResult<Self> {
        let (runtime_tx, _) = broadcast::channel(16);
        Ok(Self {
            mpv_path: settings.player.mpv.path,
            ipc_dir: settings.player.mpv.ipc_dir,
            extra_args: settings.player.mpv.extra_args,
            process: Arc::new(Mutex::new(None)),
            runtime_tx,
            current_position: Arc::new(Mutex::new(None)),
            expected_stop_generation: Arc::new(AtomicU64::new(0)),
        })
    }

    /// 为当前 generation 启动一个新的 `mpv` 子进程并建立 IPC 连接。
    ///
    /// # 参数
    /// - `path`：待播放音频文件路径
    /// - `generation`：当前播放代次
    ///
    /// # 返回值
    /// - `MeloResult<MpvProcess>`：已连接 IPC 的 `mpv` 进程句柄
    fn spawn_process(&self, path: &Path, generation: u64) -> MeloResult<MpvProcess> {
        let ipc_path = ipc_path_for_generation(&self.ipc_dir, generation);
        let mut command = build_mpv_command(&self.mpv_path, &ipc_path, &self.extra_args);
        command.arg(path);
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());

        let child = command
            .spawn()
            .map_err(|err| MeloError::Message(err.to_string()))?;
        let mut command_pipe = connect_ipc_pipe(&ipc_path)?;
        observe_property(&mut command_pipe, "time-pos")?;

        let reader = command_pipe
            .try_clone()
            .map_err(|err| MeloError::Message(err.to_string()))?;
        spawn_event_reader(
            reader,
            generation,
            self.runtime_tx.clone(),
            Arc::clone(&self.current_position),
            Arc::clone(&self.expected_stop_generation),
        );

        Ok(MpvProcess {
            child,
            command_pipe,
            ipc_path,
            generation,
        })
    }

    /// 停掉当前 `mpv` 子进程并清理 IPC 状态。
    ///
    /// # 参数
    /// - `guard`：当前受锁保护的进程状态
    ///
    /// # 返回值
    /// - `MeloResult<()>`：停止结果
    fn stop_process_locked(&self, guard: &mut Option<MpvProcess>) -> MeloResult<()> {
        stop_process_state(
            guard,
            &self.current_position,
            &self.expected_stop_generation,
        )
    }
}

impl Drop for MpvBackend {
    /// 在后端被销毁时确保没有遗留 `mpv` 子进程。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - 无
    fn drop(&mut self) {
        let mut guard = self.process.lock().unwrap();
        let _ = self.stop_process_locked(&mut guard);
    }
}

impl PlaybackBackend for MpvBackend {
    /// 返回当前后端名称。
    ///
    /// # 参数
    /// - 无
    ///
    /// # 返回值
    /// - `&'static str`：后端稳定名称
    fn backend_name(&self) -> &'static str {
        "mpv_ipc"
    }

    /// 创建并启动一个新的 `mpv` 播放会话。
    ///
    /// # 参数
    /// - `request`：播放启动参数
    ///
    /// # 返回值
    /// - `MeloResult<Box<dyn PlaybackSessionHandle>>`：单次播放控制句柄
    fn start_session(
        &self,
        request: PlaybackStartRequest,
    ) -> MeloResult<Box<dyn PlaybackSessionHandle>> {
        let mut process = self.process.lock().unwrap();
        self.stop_process_locked(&mut process)?;
        *self.current_position.lock().unwrap() = Some(Duration::from_secs(0));
        *process = Some(self.spawn_process(&request.path, request.generation)?);
        drop(process);

        let session = MpvPlaybackSession {
            process: Arc::clone(&self.process),
            runtime_tx: self.runtime_tx.clone(),
            current_position: Arc::clone(&self.current_position),
            expected_stop_generation: Arc::clone(&self.expected_stop_generation),
        };
        session.set_volume(request.volume_factor)?;
        Ok(Box::new(session))
    }
}

impl PlaybackSessionHandle for MpvPlaybackSession {
    fn pause(&self) -> MeloResult<()> {
        send_process_command(
            &self.process,
            serde_json::json!({
                "command": ["set_property", "pause", true]
            }),
        )
    }

    fn resume(&self) -> MeloResult<()> {
        send_process_command(
            &self.process,
            serde_json::json!({
                "command": ["set_property", "pause", false]
            }),
        )
    }

    fn stop(&self) -> MeloResult<()> {
        let mut process = self.process.lock().unwrap();
        stop_process_state(
            &mut process,
            &self.current_position,
            &self.expected_stop_generation,
        )
    }

    fn subscribe_runtime_events(&self) -> PlaybackRuntimeReceiver {
        self.runtime_tx.subscribe()
    }

    fn current_position(&self) -> Option<Duration> {
        *self.current_position.lock().unwrap()
    }

    fn set_volume(&self, factor: f32) -> MeloResult<()> {
        send_process_command(
            &self.process,
            serde_json::json!({
                "command": ["set_property", "volume", factor.max(0.0) * 100.0]
            }),
        )
    }
}

/// 构造一个带基础运行参数的 `mpv` 命令。
///
/// # 参数
/// - `path`：`mpv` 可执行文件路径
/// - `ipc_path`：IPC 服务路径
/// - `extra_args`：额外命令行参数
///
/// # 返回值
/// - `Command`：可继续补充参数的命令对象
pub fn build_mpv_command(path: &str, ipc_path: &str, extra_args: &[String]) -> Command {
    let mut command = Command::new(path);
    command.arg("--idle=yes");
    command.arg("--no-terminal");
    command.arg("--force-window=no");
    command.arg("--no-video");
    command.arg(format!("--input-ipc-server={ipc_path}"));
    for arg in extra_args {
        command.arg(arg);
    }
    command
}

/// 解析 `mpv` 输出的一条 JSON 事件。
///
/// # 参数
/// - `line`：原始 JSON 行
/// - `generation`：当前播放代次
///
/// # 返回值
/// - `MeloResult<Option<PlaybackRuntimeEvent>>`：识别到的运行时事件
pub fn parse_mpv_event(line: &str, generation: u64) -> MeloResult<Option<PlaybackRuntimeEvent>> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|err| MeloError::Message(err.to_string()))?;
    if value.get("event").and_then(|event| event.as_str()) == Some("end-file") {
        let reason = match value.get("reason").and_then(|reason| reason.as_str()) {
            Some("eof") => PlaybackStopReason::NaturalEof,
            Some("stop") => PlaybackStopReason::UserStop,
            Some("quit") => PlaybackStopReason::UserClosedBackend,
            _ => PlaybackStopReason::BackendAborted,
        };
        return Ok(Some(PlaybackRuntimeEvent::PlaybackStopped {
            generation,
            reason,
        }));
    }
    Ok(None)
}

/// 探测给定 `mpv` 路径是否可执行。
///
/// # 参数
/// - `path`：`mpv` 可执行文件路径或命令名
///
/// # 返回值
/// - `bool`：是否可用
pub fn mpv_exists(path: &str) -> bool {
    Command::new(path)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or_else(|_| Path::new(path).exists())
}

/// 为一个 generation 生成独立的 IPC 路径。
///
/// # 参数
/// - `ipc_dir`：配置中的 IPC 目录或特殊值
/// - `generation`：当前播放代次
///
/// # 返回值
/// - `String`：可传给 `mpv` 的 IPC 路径
fn ipc_path_for_generation(ipc_dir: &str, generation: u64) -> String {
    if cfg!(windows) {
        if ipc_dir == "auto" {
            return format!(r"\\.\pipe\melo-mpv-{}-{generation}", std::process::id());
        }

        if ipc_dir.starts_with(r"\\.\pipe\") {
            return format!("{ipc_dir}-{generation}");
        }

        return format!(r"\\.\pipe\{}-{generation}", sanitize_pipe_segment(ipc_dir));
    }

    let base = if ipc_dir == "auto" {
        std::env::temp_dir()
    } else {
        PathBuf::from(ipc_dir)
    };
    base.join(format!("melo-mpv-{generation}.sock"))
        .to_string_lossy()
        .into_owned()
}

/// 将任意字符串规范化为可用于 Windows named pipe 的片段。
///
/// # 参数
/// - `value`：原始片段
///
/// # 返回值
/// - `String`：去除路径分隔符后的安全片段
fn sanitize_pipe_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            other => other,
        })
        .collect()
}

/// 等待并打开 `mpv` 暴露出来的 IPC 管道。
///
/// # 参数
/// - `ipc_path`：IPC 路径
///
/// # 返回值
/// - `MeloResult<File>`：已打开的读写句柄
fn connect_ipc_pipe(ipc_path: &str) -> MeloResult<File> {
    let path = Path::new(ipc_path);
    let mut last_error = None;
    for _ in 0..IPC_CONNECT_ATTEMPTS {
        match OpenOptions::new().read(true).write(true).open(path) {
            Ok(file) => return Ok(file),
            Err(err) => {
                last_error = Some(err);
                std::thread::sleep(Duration::from_millis(IPC_CONNECT_DELAY_MS));
            }
        }
    }

    Err(MeloError::Message(
        last_error
            .map(|err| err.to_string())
            .unwrap_or_else(|| "mpv_ipc_unavailable".to_string()),
    ))
}

/// 请求 `mpv` 观察某个属性的变化。
///
/// # 参数
/// - `pipe`：IPC 写句柄
/// - `property`：待观察的属性名
///
/// # 返回值
/// - `MeloResult<()>`：发送结果
fn observe_property(pipe: &mut File, property: &str) -> MeloResult<()> {
    write_json_line(
        pipe,
        &serde_json::json!({
            "command": ["observe_property", 1, property]
        }),
    )
}

/// 向 IPC 写入一行 JSON 命令并立即刷新。
///
/// # 参数
/// - `pipe`：IPC 写句柄
/// - `value`：待发送的 JSON 对象
///
/// # 返回值
/// - `MeloResult<()>`：写入结果
fn write_json_line(pipe: &mut File, value: &serde_json::Value) -> MeloResult<()> {
    let mut payload =
        serde_json::to_vec(value).map_err(|err| MeloError::Message(err.to_string()))?;
    payload.push(b'\n');
    pipe.write_all(&payload)
        .and_then(|_| pipe.flush())
        .map_err(|err| MeloError::Message(err.to_string()))
}

/// 停掉当前 `mpv` 子进程并清理共享状态。
///
/// # 参数
/// - `guard`：当前受锁保护的进程状态
/// - `current_position`：共享的当前位置缓存
/// - `expected_stop_generation`：记录显式停止代次的共享状态
///
/// # 返回值
/// - `MeloResult<()>`：停止结果
fn stop_process_state(
    guard: &mut Option<MpvProcess>,
    current_position: &Arc<Mutex<Option<Duration>>>,
    expected_stop_generation: &AtomicU64,
) -> MeloResult<()> {
    if let Some(mut process) = guard.take() {
        expected_stop_generation.store(process.generation, Ordering::SeqCst);
        let _ = process.child.kill();
        let _ = process.child.wait();
        cleanup_ipc_path(&process.ipc_path);
    }
    *current_position.lock().unwrap() = None;
    Ok(())
}

/// 向共享 `mpv` 进程发送一条 JSON IPC 命令。
///
/// # 参数
/// - `process`：共享进程状态
/// - `command`：待发送的 JSON 命令
///
/// # 返回值
/// - `MeloResult<()>`：发送结果
fn send_process_command(
    process: &Arc<Mutex<Option<MpvProcess>>>,
    command: serde_json::Value,
) -> MeloResult<()> {
    let mut process = process.lock().unwrap();
    let Some(state) = process.as_mut() else {
        return Ok(());
    };
    write_json_line(&mut state.command_pipe, &command)
}

/// 启动一个后台线程读取 `mpv` IPC 事件。
///
/// # 参数
/// - `reader`：IPC 读句柄
/// - `generation`：该进程对应的播放代次
/// - `runtime_tx`：运行时事件发送器
/// - `current_position`：当前位置缓存
///
/// # 返回值
/// - 无
fn spawn_event_reader(
    reader: File,
    generation: u64,
    runtime_tx: broadcast::Sender<PlaybackRuntimeEvent>,
    current_position: Arc<Mutex<Option<Duration>>>,
    expected_stop_generation: Arc<AtomicU64>,
) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(reader);
        let mut saw_stop_event = false;
        loop {
            let mut line = String::new();
            let Ok(bytes_read) = reader.read_line(&mut line) else {
                maybe_report_backend_abort(
                    &runtime_tx,
                    &expected_stop_generation,
                    generation,
                    saw_stop_event,
                );
                break;
            };
            if bytes_read == 0 {
                maybe_report_backend_abort(
                    &runtime_tx,
                    &expected_stop_generation,
                    generation,
                    saw_stop_event,
                );
                break;
            }

            if let Some(position) = parse_playback_time_event(&line) {
                *current_position.lock().unwrap() = Some(position);
            }

            if let Ok(Some(event)) = parse_mpv_event(&line, generation) {
                saw_stop_event = true;
                let _ = runtime_tx.send(event);
            }
        }
    });
}

/// 在 IPC 提前断开且并非预期停播时，上报一次后端异常退出事件。
///
/// # 参数
/// - `runtime_tx`：运行时事件发送器
/// - `expected_stop_generation`：显式停播时记录的代次
/// - `generation`：当前读取线程对应的播放代次
/// - `saw_stop_event`：在断开前是否已经收到过显式 stop 事件
///
/// # 返回值
/// - 无
fn maybe_report_backend_abort(
    runtime_tx: &broadcast::Sender<PlaybackRuntimeEvent>,
    expected_stop_generation: &AtomicU64,
    generation: u64,
    saw_stop_event: bool,
) {
    if saw_stop_event || expected_stop_generation.load(Ordering::SeqCst) == generation {
        return;
    }

    let _ = runtime_tx.send(PlaybackRuntimeEvent::PlaybackStopped {
        generation,
        reason: PlaybackStopReason::BackendAborted,
    });
}

/// 从一条 IPC JSON 事件中提取播放位置。
///
/// # 参数
/// - `line`：原始 JSON 行
///
/// # 返回值
/// - `Option<Duration>`：解析出的播放位置
fn parse_playback_time_event(line: &str) -> Option<Duration> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("event").and_then(|event| event.as_str()) != Some("property-change") {
        return None;
    }
    let property_name = value.get("name").and_then(|name| name.as_str());
    if !matches!(property_name, Some("time-pos" | "playback-time")) {
        return None;
    }

    value
        .get("data")
        .and_then(|data| data.as_f64())
        .filter(|seconds| *seconds >= 0.0)
        .map(Duration::from_secs_f64)
}

/// 清理 IPC 路径对应的临时文件。
///
/// # 参数
/// - `ipc_path`：待清理的 IPC 路径
///
/// # 返回值
/// - 无
fn cleanup_ipc_path(ipc_path: &str) {
    if cfg!(windows) {
        return;
    }
    let _ = std::fs::remove_file(ipc_path);
}

#[cfg(test)]
mod tests;
