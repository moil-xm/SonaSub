use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use sonasub_desktop_lib::{
    engine::{EngineEvent, EngineManager, EngineState},
    protocol::{parse_handshake, parse_response, ProtocolError},
    resources::EngineResources,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

fn source_resources() -> EngineResources {
    EngineResources::development(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
}

fn wait_for<F>(manager: &EngineManager, predicate: F) -> EngineState
where
    F: Fn(&EngineState) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let state = manager.snapshot();
        if predicate(&state) {
            return state;
        }
        assert!(Instant::now() < deadline, "state did not settle: {state:?}");
        thread::sleep(Duration::from_millis(20));
    }
}

fn running(manager: &EngineManager) -> EngineState {
    wait_for(manager, |state| {
        matches!(state, EngineState::Running { .. })
    })
}

fn stopped(manager: &EngineManager) -> EngineState {
    wait_for(manager, |state| matches!(state, EngineState::Stopped))
}

fn failed(manager: &EngineManager) -> EngineState {
    wait_for(manager, |state| matches!(state, EngineState::Failed { .. }))
}

fn process_exists(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn wait_for_exit(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while process_exists(pid) {
        assert!(
            Instant::now() < deadline,
            "managed engine {pid} was not cleaned up"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

async fn control(port: u16, token: &str, id: &str, method: &str) -> Value {
    let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}"))
        .await
        .expect("connect to real engine");
    socket
        .send(Message::text(
            json!({"protocolVersion": 1, "type": "auth", "token": token}).to_string(),
        ))
        .await
        .expect("send auth");
    socket
        .send(Message::text(
            json!({"protocolVersion": 1, "type": "request", "id": id, "method": method})
                .to_string(),
        ))
        .await
        .expect("send request");
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let message = tokio::time::timeout(Duration::from_secs(2), socket.next())
            .await
            .expect("response deadline")
            .expect("socket stays open")
            .expect("valid websocket message");
        if let Message::Text(raw) = message {
            let value: Value = serde_json::from_str(raw.as_str()).expect("JSON response");
            if value["id"] == id {
                socket.close(None).await.expect("close WebSocket");
                return value;
            }
        }
    }
    panic!("no response for {id}");
}

#[test]
fn concurrent_starts_share_one_real_process() {
    let manager = EngineManager::new(source_resources());
    let mut handles = Vec::new();
    for _ in 0..8 {
        let manager = manager.clone();
        handles.push(thread::spawn(move || manager.start()));
    }
    for handle in handles {
        handle.join().expect("start task").expect("start succeeds");
    }
    let first = running(&manager);
    let first_pid = first.info().expect("real engine info").pid;
    let second = manager.start().expect("repeated start is idempotent");
    assert_eq!(second.info().expect("same engine").pid, first_pid);
    manager.shutdown();
    wait_for_exit(first_pid);
    assert!(matches!(manager.snapshot(), EngineState::Stopped));
}

#[test]
fn stop_and_restart_replace_the_managed_process_and_connection() {
    let manager = EngineManager::new(source_resources());
    manager.start().expect("first start");
    let first = running(&manager);
    let first_info = first.info().expect("first real info").clone();
    manager.stop().expect("stop request");
    stopped(&manager);
    wait_for_exit(first_info.pid);
    manager.start().expect("restart after stop");
    let second = running(&manager);
    let second_info = second.info().expect("second real info");
    assert_ne!(first_info.pid, second_info.pid);
    assert_ne!(
        first.diagnostics().expect("first endpoint").port,
        second.diagnostics().expect("second endpoint").port
    );
    assert!(second_info.node_version.starts_with("v24."));
    manager.shutdown();
    wait_for_exit(second_info.pid);
}

#[test]
fn restart_command_waits_for_the_new_running_engine() {
    let manager = EngineManager::new(source_resources());
    manager.start().expect("initial start");
    let first_pid = running(&manager).info().expect("initial info").pid;
    let state = manager.restart().expect("restart completes");
    let second_pid = state.info().expect("new info").pid;
    assert!(matches!(state, EngineState::Running { .. }));
    assert_ne!(first_pid, second_pid);
    wait_for_exit(first_pid);
    manager.shutdown();
    wait_for_exit(second_pid);
}

#[test]
fn unexpected_exit_is_failed_and_can_be_retried() {
    let manager = EngineManager::new(source_resources());
    let events = Arc::new(Mutex::new(Vec::new()));
    let received = events.clone();
    manager.set_listener(move |event| received.lock().expect("event log").push(event));
    manager.start().expect("initial start");
    let state = running(&manager);
    let pid = state.info().expect("real info").pid;
    Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .expect("kill only the PID from the owned engine handshake");
    let failure = failed(&manager);
    assert!(failure.error().expect("failure reason").contains("退出"));
    assert!(events
        .lock()
        .expect("event log")
        .iter()
        .any(|event| matches!(event, EngineEvent::Failed { .. })));
    manager.start().expect("manual retry");
    let replacement = running(&manager);
    assert_ne!(replacement.info().expect("replacement info").pid, pid);
    manager.shutdown();
    wait_for_exit(replacement.info().expect("replacement info").pid);
}

#[test]
fn wrong_authentication_cannot_query_or_stop_the_real_engine() {
    let manager = EngineManager::new(source_resources());
    manager.start().expect("start");
    let state = running(&manager);
    let port = state.diagnostics().expect("port").port;
    let pid = state.info().expect("pid").pid;
    let runtime = tokio::runtime::Runtime::new().expect("test runtime");
    runtime.block_on(async {
        let (mut socket, _) = connect_async(format!("ws://127.0.0.1:{port}"))
            .await
            .expect("connect");
        socket
            .send(Message::text(
                json!({"protocolVersion": 1, "type": "auth", "token": "wrong"}).to_string(),
            ))
            .await
            .expect("send wrong auth");
        let closed = socket
            .next()
            .await
            .expect("server closes unauthorized socket")
            .expect("close frame");
        assert!(closed.is_close());
    });
    assert!(process_exists(pid));
    let response = runtime.block_on(control(
        port,
        &manager.test_token(),
        "authorized",
        "engine.info",
    ));
    assert_eq!(response["ok"], true);
    assert_eq!(response["result"]["pid"], pid);
    manager.shutdown();
    wait_for_exit(pid);
}

#[test]
fn failed_startup_keeps_a_reason_and_leaves_no_orphan() {
    let manager = EngineManager::new(EngineResources::custom(
        PathBuf::from("/definitely/missing/node"),
        PathBuf::from("/definitely/missing/engine.mjs"),
        PathBuf::from("/definitely/missing"),
    ));
    let error = manager.start().expect_err("missing runtime must fail");
    assert!(error.contains("Node"));
    assert!(matches!(manager.snapshot(), EngineState::Failed { .. }));
    manager
        .start()
        .expect_err("manual retry still reports the real failure");
}

#[test]
fn failed_startup_reports_engine_stderr() {
    let directory =
        std::env::temp_dir().join(format!("sonasub-stderr-test-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("create stderr test directory");
    let entry = directory.join("engine.mjs");
    std::fs::write(
        &entry,
        "console.error('SONASUB_TEST_STDERR_MARKER');\nprocess.exit(7);\n",
    )
    .expect("write stderr test engine");
    let manager = EngineManager::new(EngineResources::custom(
        source_resources().node,
        entry,
        directory.clone(),
    ));
    let result = manager.start();
    std::fs::remove_dir_all(&directory).expect("remove stderr test directory");
    let error = result.expect_err("engine startup failure must be reported");
    assert!(
        error.contains("SONASUB_TEST_STDERR_MARKER"),
        "startup error lost engine stderr: {error}"
    );
    assert!(manager
        .snapshot()
        .error()
        .expect("failed state keeps a reason")
        .contains("SONASUB_TEST_STDERR_MARKER"));
}

#[test]
fn protocol_rejects_mismatched_responses_and_unrelated_logs() {
    let raw = r#"{"protocolVersion":1,"type":"response","id":"first","ok":true,"result":null}"#;
    assert!(parse_response(raw, "first", "engine.shutdown").is_ok());
    assert!(matches!(
        parse_response(raw, "second", "engine.shutdown"),
        Err(ProtocolError::RequestMismatch)
    ));
    assert!(parse_handshake("ordinary stdout log").is_err());
    assert_eq!(
        parse_handshake(r#"SONASUB_HANDSHAKE {"protocolVersion":1,"port":1234,"pid":42}"#)
            .expect("handshake")
            .pid,
        42
    );
}

struct OwnedChild(Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn application_exit_cleans_only_the_child_it_created() {
    let resources = source_resources();
    let mut unrelated = OwnedChild(
        Command::new(&resources.node)
            .args(["-e", "setInterval(() => {}, 1000)"])
            .spawn()
            .expect("unrelated Node process"),
    );
    let manager = EngineManager::new(resources);
    manager.start().expect("managed start");
    let pid = running(&manager).info().expect("managed info").pid;
    manager.shutdown();
    wait_for_exit(pid);
    assert!(process_exists(unrelated.0.id()));
    unrelated
        .0
        .kill()
        .expect("kill test-owned unrelated process");
    unrelated
        .0
        .wait()
        .expect("reap test-owned unrelated process");
}

#[test]
fn built_engine_starts_without_workspace_dependencies() {
    let resources = EngineResources::packaged(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
    let manager = EngineManager::new(resources);
    manager
        .start()
        .expect("built engine starts with bundled Node and self-contained artifact");
    let state = running(&manager);
    assert!(state
        .info()
        .expect("built engine info")
        .node_version
        .starts_with("v24."));
    assert_eq!(
        state.info().expect("built engine info").engine_version,
        "0.0.0"
    );
    manager.shutdown();
    wait_for_exit(state.info().expect("built engine info").pid);
}
