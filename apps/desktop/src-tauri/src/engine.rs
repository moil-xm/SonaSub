use std::{
    collections::VecDeque,
    process::Stdio,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use tauri::async_runtime::spawn_blocking;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    runtime::Runtime,
    task::JoinHandle,
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::{
    protocol::{self, EngineInfo, Handshake},
    resources::EngineResources,
};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
type Listener = Arc<dyn Fn(EngineEvent) + Send + Sync>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineDiagnostics {
    pub pid: u32,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum EngineState {
    Stopped,
    Starting { pid: Option<u32> },
    Running { info: EngineInfo, port: u16 },
    Stopping { info: EngineInfo, port: u16 },
    Failed { error: String },
}

impl EngineState {
    pub fn info(&self) -> Option<&EngineInfo> {
        match self {
            Self::Running { info, .. } | Self::Stopping { info, .. } => Some(info),
            _ => None,
        }
    }

    pub fn diagnostics(&self) -> Option<EngineDiagnostics> {
        match self {
            Self::Running { info, port } | Self::Stopping { info, port } => {
                Some(EngineDiagnostics {
                    pid: info.pid,
                    port: *port,
                })
            }
            _ => None,
        }
    }

    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Failed { error } => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    Starting { pid: Option<u32> },
    Running { info: EngineInfo, port: u16 },
    Stopping { info: EngineInfo, port: u16 },
    Stopped,
    Failed { error: String },
}

impl From<&EngineState> for EngineEvent {
    fn from(state: &EngineState) -> Self {
        match state {
            EngineState::Stopped => Self::Stopped,
            EngineState::Starting { pid } => Self::Starting { pid: *pid },
            EngineState::Running { info, port } => Self::Running {
                info: info.clone(),
                port: *port,
            },
            EngineState::Stopping { info, port } => Self::Stopping {
                info: info.clone(),
                port: *port,
            },
            EngineState::Failed { error } => Self::Failed {
                error: error.clone(),
            },
        }
    }
}

struct Inner {
    runtime: Runtime,
    resources: EngineResources,
    state: Mutex<EngineState>,
    transition: Mutex<()>,
    child: Mutex<Option<Child>>,
    socket: Mutex<Option<Socket>>,
    token: Mutex<Option<String>>,
    stderr: Arc<Mutex<VecDeque<String>>>,
    stderr_task: Mutex<Option<JoinHandle<()>>>,
    request_id: AtomicU64,
    listener: Mutex<Option<Listener>>,
}

#[derive(Clone)]
pub struct EngineManager {
    inner: Arc<Inner>,
}

impl EngineManager {
    pub fn new(resources: EngineResources) -> Self {
        Self {
            inner: Arc::new(Inner {
                runtime: Runtime::new().expect("Tokio runtime for the local engine"),
                resources,
                state: Mutex::new(EngineState::Stopped),
                transition: Mutex::new(()),
                child: Mutex::new(None),
                socket: Mutex::new(None),
                token: Mutex::new(None),
                stderr: Arc::new(Mutex::new(VecDeque::new())),
                stderr_task: Mutex::new(None),
                request_id: AtomicU64::new(1),
                listener: Mutex::new(None),
            }),
        }
    }

    pub fn set_listener(&self, listener: impl Fn(EngineEvent) + Send + Sync + 'static) {
        *self.inner.listener.lock().expect("listener lock") = Some(Arc::new(listener));
    }

    pub fn snapshot(&self) -> EngineState {
        self.inner.state.lock().expect("state lock").clone()
    }

    #[doc(hidden)]
    pub fn test_token(&self) -> String {
        self.inner
            .token
            .lock()
            .expect("token lock")
            .clone()
            .expect("running test engine has a token")
    }

    pub fn start(&self) -> Result<EngineState, String> {
        let _transition = self.inner.transition.lock().expect("transition lock");
        let state = self.snapshot();
        if matches!(state, EngineState::Running { .. }) {
            return Ok(state);
        }
        if matches!(state, EngineState::Stopping { .. }) {
            return Err("引擎正在停止，请稍后再启动。".to_owned());
        }
        self.set_state(EngineState::Starting { pid: None });
        let result = self.inner.runtime.block_on(self.start_process());
        match result {
            Ok(state) => Ok(state),
            Err(error) => {
                self.inner.runtime.block_on(self.cleanup_child());
                self.inner.runtime.block_on(self.drain_stderr());
                *self.inner.socket.lock().expect("socket lock") = None;
                *self.inner.token.lock().expect("token lock") = None;
                let error = self.with_stderr(error);
                self.set_state(EngineState::Failed {
                    error: error.clone(),
                });
                Err(error)
            }
        }
    }

    pub fn stop(&self) -> Result<EngineState, String> {
        let _transition = self.inner.transition.lock().expect("transition lock");
        let state = self.snapshot();
        match state {
            EngineState::Running { info, port } => {
                self.set_state(EngineState::Stopping {
                    info: info.clone(),
                    port,
                });
                self.inner.runtime.block_on(self.stop_process(info.pid))?;
                self.set_state(EngineState::Stopped);
                Ok(self.snapshot())
            }
            EngineState::Stopped | EngineState::Failed { .. } => Ok(state),
            EngineState::Starting { .. } => {
                Err("引擎正在启动，请等待启动完成后再停止。".to_owned())
            }
            EngineState::Stopping { .. } => Ok(state),
        }
    }

    pub fn restart(&self) -> Result<EngineState, String> {
        self.stop()?;
        self.start()
    }

    pub fn shutdown(&self) {
        let _transition = self.inner.transition.lock().expect("transition lock");
        let pid = self.snapshot().info().map(|info| info.pid);
        let socket = self.inner.socket.lock().expect("socket lock").take();
        self.inner.runtime.block_on(async {
            if let Some(mut socket) = socket {
                let _ = socket.close(None).await;
            }
            self.cleanup_child().await;
        });
        *self.inner.token.lock().expect("token lock") = None;
        if pid.is_some() {
            self.set_state(EngineState::Stopped);
        }
    }

    fn set_state(&self, state: EngineState) {
        let event = EngineEvent::from(&state);
        *self.inner.state.lock().expect("state lock") = state;
        if let Some(listener) = self.inner.listener.lock().expect("listener lock").clone() {
            listener(event);
        }
    }

    async fn start_process(&self) -> Result<EngineState, String> {
        let token = random_token()?;
        let mut command = Command::new(&self.inner.resources.node);
        command
            .arg(&self.inner.resources.entry)
            .current_dir(&self.inner.resources.working_directory)
            .env("SONASUB_ENGINE_TOKEN", &token)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|error| format!("无法启动 Node 运行时：{error}"))?;
        let pid = child
            .id()
            .ok_or_else(|| "引擎进程已提前退出。".to_owned())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "无法读取引擎错误输出。".to_owned())?;
        self.set_state(EngineState::Starting { pid: Some(pid) });
        *self.inner.child.lock().expect("child lock") = Some(child);
        self.inner.stderr.lock().expect("stderr lock").clear();
        let stderr_buffer = self.inner.stderr.clone();
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                eprintln!("[engine] {line}");
                let mut recent = stderr_buffer.lock().expect("stderr lock");
                if recent.len() >= 8 {
                    recent.pop_front();
                }
                recent.push_back(line);
            }
        });
        *self.inner.stderr_task.lock().expect("stderr task lock") = Some(stderr_task);

        let handshake = self.read_handshake(pid).await?;
        let mut socket = self.connect(handshake.port).await?;
        socket
            .send(Message::text(protocol::auth_message(&token)))
            .await
            .map_err(|error| format!("发送引擎认证失败：{error}"))?;
        let info = self
            .request(&mut socket, "engine.info")
            .await?
            .ok_or_else(|| "引擎未返回运行信息。".to_owned())?;
        if info.pid != handshake.pid {
            return Err("引擎握手与运行信息的进程 ID 不一致。".to_owned());
        }
        if !info.node_version.starts_with("v24.") {
            return Err(format!(
                "引擎需要 Node.js 24，实际为 {}。",
                info.node_version
            ));
        }
        *self.inner.socket.lock().expect("socket lock") = Some(socket);
        *self.inner.token.lock().expect("token lock") = Some(token);
        let state = EngineState::Running {
            info,
            port: handshake.port,
        };
        self.set_state(state.clone());
        self.watch_process(pid);
        Ok(state)
    }

    async fn read_handshake(&self, pid: u32) -> Result<Handshake, String> {
        let stdout = self
            .inner
            .child
            .lock()
            .expect("child lock")
            .as_mut()
            .and_then(|child| child.stdout.take())
            .ok_or_else(|| "无法读取引擎启动输出。".to_owned())?;
        let mut lines = BufReader::new(stdout).lines();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err("引擎启动握手超时。".to_owned());
            }
            let line = tokio::time::timeout(remaining, lines.next_line())
                .await
                .map_err(|_| "引擎启动握手超时。".to_owned())?
                .map_err(|error| format!("读取引擎启动输出失败：{error}"))?;
            let Some(line) = line else {
                return Err("引擎在握手完成前退出。".to_owned());
            };
            if let Ok(handshake) = protocol::parse_handshake(&line) {
                if handshake.pid != pid {
                    return Err("引擎握手进程 ID 与受管理进程不一致。".to_owned());
                }
                tokio::spawn(async move {
                    while let Ok(Some(line)) = lines.next_line().await {
                        eprintln!("[engine] {line}");
                    }
                });
                return Ok(handshake);
            }
            eprintln!("[engine] {line}");
        }
    }

    async fn connect(&self, port: u16) -> Result<Socket, String> {
        let (socket, _) = tokio::time::timeout(
            Duration::from_secs(3),
            connect_async(format!("ws://127.0.0.1:{port}")),
        )
        .await
        .map_err(|_| "连接本机引擎超时。".to_owned())?
        .map_err(|error| format!("连接本机引擎失败：{error}"))?;
        Ok(socket)
    }

    async fn request(
        &self,
        socket: &mut Socket,
        method: &str,
    ) -> Result<Option<EngineInfo>, String> {
        let id = format!(
            "req-{}",
            self.inner.request_id.fetch_add(1, Ordering::Relaxed)
        );
        socket
            .send(Message::text(protocol::request_message(&id, method)))
            .await
            .map_err(|error| format!("发送引擎请求失败：{error}"))?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err("引擎响应超时。".to_owned());
            }
            let message = tokio::time::timeout(remaining, socket.next())
                .await
                .map_err(|_| "引擎响应超时。".to_owned())?
                .ok_or_else(|| "引擎连接已关闭。".to_owned())?
                .map_err(|error| format!("读取引擎响应失败：{error}"))?;
            match message {
                Message::Text(raw) => {
                    return protocol::parse_response(raw.as_str(), &id, method)
                        .map_err(|error| error.to_string());
                }
                Message::Close(_) => return Err("引擎连接已关闭。".to_owned()),
                Message::Ping(_) | Message::Pong(_) => continue,
                _ => return Err("引擎响应不是文本协议消息。".to_owned()),
            }
        }
    }

    async fn stop_process(&self, pid: u32) -> Result<(), String> {
        let mut socket = self.inner.socket.lock().expect("socket lock").take();
        if let Some(socket) = socket.as_mut() {
            let _ = self.request(socket, "engine.shutdown").await;
            let _ = socket.close(None).await;
        }
        *self.inner.token.lock().expect("token lock") = None;
        self.wait_or_kill(pid).await;
        Ok(())
    }

    async fn wait_or_kill(&self, pid: u32) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        loop {
            let exited = {
                let mut guard = self.inner.child.lock().expect("child lock");
                match guard.as_mut() {
                    Some(child) if child.id() == Some(pid) => child
                        .try_wait()
                        .map_err(|error| error.to_string())
                        .ok()
                        .flatten()
                        .is_some(),
                    _ => true,
                }
            };
            if exited {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                let mut guard = self.inner.child.lock().expect("child lock");
                if let Some(child) = guard.as_mut() {
                    let _ = child.start_kill();
                }
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        self.cleanup_child().await;
    }

    async fn cleanup_child(&self) {
        let child = self.inner.child.lock().expect("child lock").take();
        if let Some(mut child) = child {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }

    async fn drain_stderr(&self) {
        let task = self
            .inner
            .stderr_task
            .lock()
            .expect("stderr task lock")
            .take();
        if let Some(task) = task {
            let _ = tokio::time::timeout(Duration::from_secs(1), task).await;
        }
    }

    fn with_stderr(&self, error: String) -> String {
        let recent = self.inner.stderr.lock().expect("stderr lock");
        if recent.is_empty() {
            return error;
        }
        format!(
            "{error}\n{}",
            recent.iter().cloned().collect::<Vec<_>>().join("\n")
        )
    }

    fn watch_process(&self, pid: u32) {
        let manager = self.clone();
        self.inner.runtime.spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let status = {
                    let mut guard = manager.inner.child.lock().expect("child lock");
                    match guard.as_mut() {
                        Some(child) if child.id() == Some(pid) => child.try_wait().ok().flatten(),
                        _ => return,
                    }
                };
                if let Some(status) = status {
                    let current = manager.snapshot();
                    if current.info().is_some_and(|info| info.pid == pid)
                        && !matches!(current, EngineState::Stopping { .. })
                    {
                        *manager.inner.socket.lock().expect("socket lock") = None;
                        *manager.inner.token.lock().expect("token lock") = None;
                        manager.set_state(EngineState::Failed {
                            error: format!("引擎意外退出（{status}）。"),
                        });
                    }
                    return;
                }
            }
        });
    }
}

fn random_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|error| format!("生成引擎认证信息失败：{error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[tauri::command]
pub async fn start_command(
    manager: tauri::State<'_, EngineManager>,
) -> Result<EngineState, String> {
    let manager = manager.inner().clone();
    spawn_blocking(move || manager.start())
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn stop_command(manager: tauri::State<'_, EngineManager>) -> Result<EngineState, String> {
    let manager = manager.inner().clone();
    spawn_blocking(move || manager.stop())
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn restart_command(
    manager: tauri::State<'_, EngineManager>,
) -> Result<EngineState, String> {
    let manager = manager.inner().clone();
    spawn_blocking(move || manager.restart())
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn status_command(manager: tauri::State<'_, EngineManager>) -> EngineState {
    manager.snapshot()
}
