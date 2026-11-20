mod captions;
mod capture;
mod startup;
mod translation;

use std::{
    mem,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use crossbeam_channel::{tick, Receiver, Sender};
use sysinfo::{Pid, ProcessesToUpdate, System};

use self::startup::StartupError;
use super::{
    manager::ManagerInner,
    state::{self, add_notice, finish_session, publish_failed, publish_status, publish_stopped},
    types::{SessionInfo, SessionStatus},
};
use crate::{
    asr::AsrInput,
    audio::CaptureEvent,
    config::AppConfig,
    pipeline::{
        audio_flow::AudioFlow,
        lifecycle,
        workers::{ShutdownState, WorkerEvent, WorkerKind, WorkerRuntime},
    },
    subtitles::SubtitlePatch,
};

const UI_BATCH_INTERVAL: Duration = Duration::from_millis(80);
const STATS_INTERVAL: Duration = Duration::from_millis(750);

/// How often the session reports that it is still waiting for native work
/// that cannot be interrupted.
const NATIVE_WAIT_NOTICE_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub(super) struct RunnerContext {
    pub(super) inner: Arc<ManagerInner>,
    pub(super) session_id: String,
    pub(super) config: AppConfig,
    pub(super) config_revision: u64,
    pub(super) stop: Arc<AtomicBool>,
    pub(super) started_at_ms: u64,
    pub(super) history_limit: usize,
}

pub(super) fn session_thread(context: RunnerContext, ready: mpsc::Sender<Result<(), String>>) {
    match Runner::initialize(context.clone()) {
        Ok(runner) => {
            if ready.send(Ok(())).is_err() {
                context.stop.store(true, Ordering::SeqCst);
            }
            let failure = runner.run();
            finish_session(&context.inner, failure.as_deref());
        }
        Err(StartupError::Cancelled) => {
            publish_stopped(&context.inner, Some("Session start was cancelled."));
            let _ = ready.send(Err("Session start was cancelled.".to_owned()));
        }
        Err(StartupError::Failed(error)) => {
            publish_failed(&context.inner, &error);
            let _ = ready.send(Err(error));
        }
    }
}

struct Runner {
    inner: Arc<ManagerInner>,
    config: AppConfig,
    config_revision: u64,
    session_id: String,
    stop: Arc<AtomicBool>,
    info: SessionInfo,
    capture_rx: Receiver<CaptureEvent>,
    _capture_keepalive: Option<Sender<CaptureEvent>>,
    capture_handle: Option<JoinHandle<()>>,
    workers: WorkerRuntime,
    audio: AudioFlow,
    pending_patches: Vec<SubtitlePatch>,
    last_patch_emit: Instant,
    last_stats_emit: Instant,
    last_diarization: Instant,
    shutdown_started: Option<Instant>,
    last_wait_notice: Instant,
    ending: bool,
    failure: Option<String>,
    shutdown: ShutdownState,
    resources: ResourceMonitor,
}

impl Runner {
    fn run(mut self) -> Option<String> {
        let ticker = tick(Duration::from_millis(50));
        loop {
            crossbeam_channel::select! {
                recv(self.capture_rx) -> message => {
                    match message {
                        Ok(event) => self.handle_capture(event),
                        Err(_) => {
                            if !self.ending {
                                if self.stop.load(Ordering::SeqCst) {
                                    self.begin_shutdown(None);
                                } else {
                                    self.begin_shutdown(Some(format!(
                                        "{} capture ended unexpectedly.",
                                        self.info.audio.source_kind.label()
                                    )));
                                }
                            }
                        }
                    }
                }
                recv(self.workers.events) -> message => {
                    match message {
                        Ok(event) => self.handle_worker_event(event),
                        Err(_) => {
                            if !self.ending {
                                self.failure = Some(
                                    "All native workers exited before the session stopped.".to_owned(),
                                );
                            }
                            break;
                        }
                    }
                }
                recv(ticker) -> _ => self.handle_tick(),
            }
            // The translation queue may only close once the recognition
            // worker stopped and every subtitle it produced was consumed.
            // Closing it earlier would drop the translation of the last
            // subtitle.
            if self.ending && self.shutdown.may_close_translation(&self.workers.events) {
                self.workers.queue.close();
                self.shutdown.mark_translation_closed();
            }
            if self.ending && self.shutdown.all_finished() && self.workers.events.is_empty() {
                break;
            }
        }
        self.flush_patches(true);
        self.publish_stats();
        if let Some(handle) = self.capture_handle.take() {
            let _ = handle.join();
        }
        let failure = self.failure.clone();
        let workers = mem::replace(&mut self.workers, WorkerRuntime::empty());
        workers.join();
        failure
    }

    fn handle_worker_event(&mut self, event: WorkerEvent) {
        match event {
            WorkerEvent::Asr(event) => self.handle_asr_event(event),
            WorkerEvent::Speaker(result) => self.handle_speaker_result(result),
            WorkerEvent::Diarization(result) => self.handle_diarization_result(result),
            WorkerEvent::Translation(result) => self.handle_translation_result(result),
            WorkerEvent::Done(kind) => self.mark_worker_done(kind),
        }
    }

    fn mark_worker_done(&mut self, kind: WorkerKind) {
        match kind {
            WorkerKind::Asr => {}
            WorkerKind::Speaker => {
                self.workers.speaker_tx = None;
            }
            WorkerKind::Diarization => {
                self.workers.diarization_tx = None;
            }
            WorkerKind::Translation => {}
        }
        self.shutdown.worker_done(kind);
    }

    fn handle_tick(&mut self) {
        for kind in self.workers.finished_workers() {
            // A worker that exited without reporting its completion left a
            // pending event in the queue that explains why. The failure is
            // only reported once the queue is empty.
            if !self.ending && self.workers.events.is_empty() {
                self.begin_shutdown(Some(format!(
                    "The {} worker exited unexpectedly.",
                    kind.label()
                )));
            }
            self.mark_worker_done(kind);
        }
        if self.stop.load(Ordering::SeqCst) && !self.ending {
            self.begin_shutdown(None);
        }
        self.report_native_wait();
        if !self.ending {
            self.schedule_stable_translations();
            self.schedule_diarization();
            self.update_resource_stats();
        }
        self.flush_patches(false);
        if self.last_stats_emit.elapsed() >= STATS_INTERVAL {
            self.publish_stats();
        }
    }

    /// Reports that the session is still waiting for native work.
    ///
    /// A native inference call cannot be interrupted, so the session keeps
    /// waiting instead of dropping the worker handle and pretending the
    /// cleanup finished.
    fn report_native_wait(&mut self) {
        let Some(started) = self.shutdown_started else {
            return;
        };
        if started.elapsed() < NATIVE_WAIT_NOTICE_INTERVAL
            || self.last_wait_notice.elapsed() < NATIVE_WAIT_NOTICE_INTERVAL
        {
            return;
        }
        self.last_wait_notice = Instant::now();
        add_notice(
            &self.inner,
            "info",
            "Still waiting for the native speech runtime to return. Native inference cannot be interrupted, so the session keeps the worker until it finishes."
                .to_owned(),
        );
    }

    fn update_resource_stats(&mut self) {
        if self.last_stats_emit.elapsed() < STATS_INTERVAL {
            return;
        }
        if let Some((cpu, rss)) = self.resources.sample() {
            let mut runtime = self.inner.runtime();
            runtime.metrics.set_resources(cpu, rss);
        }
    }

    fn begin_shutdown(&mut self, failure: Option<String>) {
        if self.ending {
            return;
        }
        self.ending = true;
        self.failure = failure;
        self.shutdown_started = Some(Instant::now());
        self.stop.store(true, Ordering::SeqCst);
        publish_status(
            &self.inner,
            SessionStatus::Stopping {
                session_id: self.session_id.clone(),
                stage: if self.failure.is_some() {
                    "Stopping after a pipeline failure".to_owned()
                } else {
                    "Flushing recognition and waiting for native inference".to_owned()
                },
            },
        );
        let pending_capture = lifecycle::stop_capture(
            &mut self.capture_handle,
            &mut self.capture_rx,
            &mut self._capture_keepalive,
        );
        self.consume_pending_capture(pending_capture);
        let asr_tail = self.finish_audio();
        lifecycle::stop_workers(&mut self.workers, asr_tail);
    }

    fn consume_pending_capture(&mut self, events: Vec<CaptureEvent>) {
        for event in events {
            if let CaptureEvent::Chunk(chunk) = event {
                self.process_chunk(chunk);
            }
        }
    }

    fn finish_audio(&mut self) -> Vec<AsrInput> {
        let mut tail = Vec::new();
        if let Some(at_secs) = self.audio.pending_discontinuity() {
            tail.push(AsrInput::Discontinuity { at_secs });
            self.audio.clear_pending_discontinuity();
        }
        if let Some(outcome) = self.audio.finish() {
            tail.push(AsrInput::Audio {
                samples: outcome.samples,
                start_frame_16k: outcome.start_frame_16k,
            });
        }
        tail
    }

    fn flush_patches(&mut self, force: bool) {
        if self.pending_patches.is_empty() {
            return;
        }
        if !force && self.last_patch_emit.elapsed() < UI_BATCH_INTERVAL {
            return;
        }
        let patches = mem::take(&mut self.pending_patches);
        state::publish_subtitles(&self.inner, &self.session_id, patches);
        self.last_patch_emit = Instant::now();
    }

    fn publish_stats(&mut self) {
        state::publish_stats(&self.inner);
        self.last_stats_emit = Instant::now();
    }
}

struct ResourceMonitor {
    system: Option<System>,
    pid: Pid,
    last_sample: Instant,
}

impl ResourceMonitor {
    fn new() -> Self {
        Self {
            system: System::new().ok(),
            pid: Pid::from_u32(std::process::id()),
            last_sample: Instant::now() - Duration::from_secs(10),
        }
    }

    fn sample(&mut self) -> Option<(f32, f64)> {
        if self.last_sample.elapsed() < Duration::from_secs(1) {
            return None;
        }
        self.last_sample = Instant::now();
        let system = self.system.as_mut()?;
        system.refresh_processes(ProcessesToUpdate::Some(&[self.pid]), true);
        let process = system.process(self.pid)?;
        Some((process.cpu_usage(), process.memory() as f64 / 1_048_576.0))
    }
}
