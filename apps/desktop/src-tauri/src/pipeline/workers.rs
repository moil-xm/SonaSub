//! The native worker threads used by live sessions and recording sources.
//!
//! The recognition, speaker, diarization and translation work runs on its own
//! threads. This module owns their lifecycle so the stop protocol can be
//! verified without the acoustic models.

use std::{
    sync::{mpsc, Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crossbeam_channel::{bounded, Receiver, Sender};

use crate::{
    asr::{AsrControl, AsrEvent, AsrInput, Recognition},
    models::{ModelPaths, TranslationModelSpec, BERGAMOT_ENGINE_ID},
    speaker::{DiarizationEngine, SpeakerEngine, SpeakerInfo, SpeakerRegistry},
    translation::{
        create_engine, run_worker, TranslationEngine, TranslationQueue, TranslationResult,
    },
};

pub const ASR_QUEUE_DEPTH: usize = 128;
pub const SPEAKER_QUEUE_DEPTH: usize = 8;
pub const DIARIZATION_QUEUE_DEPTH: usize = 2;
pub const WORKER_EVENT_QUEUE_DEPTH: usize = 1024;
const TRANSLATION_LOAD_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerKind {
    Asr,
    Speaker,
    Diarization,
    Translation,
}

impl WorkerKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Asr => "ASR",
            Self::Speaker => "speaker",
            Self::Diarization => "diarization",
            Self::Translation => "translation",
        }
    }
}

pub enum WorkerEvent {
    Asr(AsrEvent),
    Speaker(SpeakerResult),
    Diarization(DiarizationResult),
    Translation(TranslationResult),
    Done(WorkerKind),
}

pub struct SpeakerTask {
    pub segment_id: String,
    pub samples: Arc<Vec<f32>>,
    pub queued_at: Instant,
}

pub enum SpeakerInput {
    Task(SpeakerTask),
    Stop,
}

pub struct SpeakerResult {
    pub segment_id: String,
    pub result: Result<Option<SpeakerInfo>, String>,
    pub latency_ms: u64,
}

pub struct DiarizationTask {
    pub start_secs: f64,
    pub samples: Arc<Vec<f32>>,
}

pub enum DiarizationInput {
    Task(DiarizationTask),
    Stop,
}

pub struct DiarizationResult {
    pub result: Result<crate::speaker::DiarizationWindow, String>,
}

/// Tracks how far the shutdown of a session progressed.
///
/// The translation queue may only close once the recognition worker stopped
/// and every subtitle it produced was consumed. Closing it earlier would
/// drop the translation of the last subtitle, so the queue closing is a
/// separate step instead of a side effect of the recognition worker.
#[derive(Debug, Default)]
pub struct ShutdownState {
    asr_done: bool,
    speaker_done: bool,
    diarization_done: bool,
    translation_done: bool,
    translation_closed: bool,
}

impl ShutdownState {
    pub fn new(speakers_enabled: bool, diarization_enabled: bool) -> Self {
        Self {
            asr_done: false,
            speaker_done: !speakers_enabled,
            diarization_done: !diarization_enabled,
            translation_done: false,
            translation_closed: false,
        }
    }

    pub fn worker_done(&mut self, kind: WorkerKind) {
        match kind {
            WorkerKind::Asr => self.asr_done = true,
            WorkerKind::Speaker => self.speaker_done = true,
            WorkerKind::Diarization => self.diarization_done = true,
            WorkerKind::Translation => self.translation_done = true,
        }
    }

    pub fn all_finished(&self) -> bool {
        self.asr_done && self.speaker_done && self.diarization_done && self.translation_done
    }

    /// Reports whether the translation queue may close now.
    ///
    /// The queue may only close once the recognition worker stopped and the
    /// session consumed every event it produced. A subtitle that is still
    /// queued would otherwise lose its translation, and a queue that never
    /// closes keeps the translation worker waiting forever.
    ///
    /// The event queue is passed in instead of a flag so that the emptiness
    /// check cannot be inverted at the call site.
    pub fn may_close_translation(&self, events: &Receiver<WorkerEvent>) -> bool {
        self.asr_done && !self.translation_closed && events.is_empty()
    }

    pub fn mark_translation_closed(&mut self) {
        self.translation_closed = true;
    }

    pub fn translation_closed(&self) -> bool {
        self.translation_closed
    }
}

pub struct WorkerRuntime {
    pub events: Receiver<WorkerEvent>,
    pub asr_tx: Option<Sender<AsrInput>>,
    pub asr_control_tx: Option<Sender<AsrControl>>,
    pub speaker_tx: Option<Sender<SpeakerInput>>,
    pub diarization_tx: Option<Sender<DiarizationInput>>,
    pub queue: Arc<TranslationQueue>,
    handles: Vec<(WorkerKind, JoinHandle<()>)>,
}

impl WorkerRuntime {
    pub fn spawn(
        asr_engine: Box<dyn Recognition>,
        models: ModelPaths,
        translation_spec: &'static TranslationModelSpec,
        speaker_engine: Option<SpeakerEngine>,
        diarization_engine: Option<DiarizationEngine>,
        registry: Option<Arc<Mutex<SpeakerRegistry>>>,
    ) -> Result<Self, String> {
        let (event_tx, event_rx) = bounded(WORKER_EVENT_QUEUE_DEPTH);
        let (asr_tx, asr_rx) = bounded(ASR_QUEUE_DEPTH);
        let (asr_control_tx, asr_control_rx) = bounded(1);
        let (speaker_tx, speaker_rx) = bounded(SPEAKER_QUEUE_DEPTH);
        let (diarization_tx, diarization_rx) = bounded(DIARIZATION_QUEUE_DEPTH);
        let queue = Arc::new(TranslationQueue::new());
        let mut handles = Vec::new();
        let mut optional_speaker_tx = None;
        let mut optional_diarization_tx = None;

        let asr_events = event_tx.clone();
        let handle = thread::Builder::new()
            .name("sonasub-asr".to_owned())
            .spawn(move || run_asr_worker(asr_engine, asr_rx, asr_control_rx, asr_events))
            .map_err(|error| format!("Failed to start the ASR worker: {error}"))?;
        handles.push((WorkerKind::Asr, handle));

        if let (Some(engine), Some(registry)) = (speaker_engine, registry.clone()) {
            let speaker_events = event_tx.clone();
            let handle = thread::Builder::new()
                .name("sonasub-speaker".to_owned())
                .spawn(move || run_speaker_worker(engine, registry, speaker_rx, speaker_events))
                .map_err(|error| format!("Failed to start the speaker worker: {error}"))?;
            handles.push((WorkerKind::Speaker, handle));
            optional_speaker_tx = Some(speaker_tx);
        }

        if let (Some(engine), Some(registry)) = (diarization_engine, registry) {
            let diarization_events = event_tx.clone();
            let handle = thread::Builder::new()
                .name("sonasub-diarization".to_owned())
                .spawn(move || {
                    run_diarization_worker(engine, registry, diarization_rx, diarization_events)
                })
                .map_err(|error| format!("Failed to start the diarization worker: {error}"))?;
            handles.push((WorkerKind::Diarization, handle));
            optional_diarization_tx = Some(diarization_tx);
        }

        let translation_events = event_tx.clone();
        let translation_queue = queue.clone();
        let (load_tx, load_rx) = mpsc::channel::<Result<(), String>>();
        let handle = thread::Builder::new()
            .name("sonasub-translation".to_owned())
            .spawn(move || {
                match create_engine(BERGAMOT_ENGINE_ID, &models, translation_spec) {
                    Ok(engine) => {
                        let _ = load_tx.send(Ok(()));
                        run_worker(engine, &translation_queue, |result| {
                            let _ = translation_events.send(WorkerEvent::Translation(result));
                        });
                    }
                    Err(error) => {
                        let _ = load_tx.send(Err(error));
                    }
                }
                let _ = translation_events.send(WorkerEvent::Done(WorkerKind::Translation));
            })
            .map_err(|error| format!("Failed to start the translation worker: {error}"))?;
        handles.push((WorkerKind::Translation, handle));
        drop(event_tx);
        let workers = Self {
            events: event_rx,
            asr_tx: Some(asr_tx),
            asr_control_tx: Some(asr_control_tx),
            speaker_tx: optional_speaker_tx,
            diarization_tx: optional_diarization_tx,
            queue,
            handles,
        };
        match load_rx.recv_timeout(TRANSLATION_LOAD_TIMEOUT) {
            Ok(Ok(())) => Ok(workers),
            Ok(Err(error)) => {
                workers.shutdown();
                Err(error)
            }
            Err(error) => {
                workers.shutdown();
                Err(format!(
                    "The Bergamot translation worker did not load the model: {error}"
                ))
            }
        }
    }

    pub fn empty() -> Self {
        let (_, event_rx) = bounded(WORKER_EVENT_QUEUE_DEPTH);
        Self {
            events: event_rx,
            asr_tx: None,
            asr_control_tx: None,
            speaker_tx: None,
            diarization_tx: None,
            queue: Arc::new(TranslationQueue::new()),
            handles: Vec::new(),
        }
    }

    /// Releases every worker without waiting for a graceful flush.
    ///
    /// The call is used on the failure paths of the session start, where no
    /// subtitle may be produced any more.
    pub fn shutdown(mut self) {
        self.queue.close();
        self.asr_tx = None;
        self.asr_control_tx = None;
        self.speaker_tx = None;
        self.diarization_tx = None;
        self.drain_and_join();
    }

    pub fn join(mut self) {
        self.drain_and_join();
    }

    /// Joins the workers while the event channel keeps being drained.
    ///
    /// A worker that blocks on a full event channel would never reach its
    /// join, so the queue is emptied on a separate thread until every worker
    /// released its sender.
    fn drain_and_join(&mut self) {
        let events = self.events.clone();
        let drainer = thread::Builder::new()
            .name("sonasub-worker-drain".to_owned())
            .spawn(move || while events.recv().is_ok() {})
            .ok();
        for (_, handle) in self.handles.drain(..) {
            let _ = handle.join();
        }
        if let Some(drainer) = drainer {
            let _ = drainer.join();
        }
    }

    pub fn finished_workers(&self) -> Vec<WorkerKind> {
        self.handles
            .iter()
            .filter(|(_, handle)| handle.is_finished())
            .map(|(kind, _)| *kind)
            .collect()
    }
}

fn run_asr_worker(
    mut engine: Box<dyn Recognition>,
    input: Receiver<AsrInput>,
    control: Receiver<AsrControl>,
    events: Sender<WorkerEvent>,
) {
    loop {
        let mut emit = |event| {
            let _ = events.send(WorkerEvent::Asr(event));
        };
        // The control channel is checked first so a stop request is never
        // delayed by a full audio queue.
        let message = crossbeam_channel::select_biased! {
            recv(control) -> message => match message {
                Ok(AsrControl::Stop) => {
                    if let Err(error) = engine.flush(&mut emit) {
                        emit(AsrEvent::Fatal(error));
                    }
                    break;
                }
                Err(_) => break,
            },
            recv(input) -> message => match message {
                Ok(message) => message,
                Err(_) => break,
            },
        };
        let result = match message {
            AsrInput::Audio {
                samples,
                start_frame_48k,
            } => engine.push(&samples, start_frame_48k, &mut emit),
            AsrInput::Discontinuity { at_secs } => engine.discontinuity(at_secs, &mut emit),
        };
        if let Err(error) = result {
            emit(AsrEvent::Fatal(error));
            break;
        }
    }
    let _ = events.send(WorkerEvent::Done(WorkerKind::Asr));
}

fn run_speaker_worker(
    mut engine: SpeakerEngine,
    registry: Arc<Mutex<SpeakerRegistry>>,
    input: Receiver<SpeakerInput>,
    events: Sender<WorkerEvent>,
) {
    while let Ok(message) = input.recv() {
        let SpeakerInput::Task(task) = message else {
            break;
        };
        let result = engine.embedding(&task.samples).map(|embedding| {
            let mut registry = registry
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            registry.identify(&embedding, true).0
        });
        let _ = events.send(WorkerEvent::Speaker(SpeakerResult {
            segment_id: task.segment_id,
            result,
            latency_ms: task.queued_at.elapsed().as_millis() as u64,
        }));
    }
    let _ = events.send(WorkerEvent::Done(WorkerKind::Speaker));
}

fn run_diarization_worker(
    mut engine: DiarizationEngine,
    registry: Arc<Mutex<SpeakerRegistry>>,
    input: Receiver<DiarizationInput>,
    events: Sender<WorkerEvent>,
) {
    while let Ok(message) = input.recv() {
        let DiarizationInput::Task(task) = message else {
            break;
        };
        let result = engine.analyze(task.start_secs, task.samples, &registry);
        let _ = events.send(WorkerEvent::Diarization(DiarizationResult { result }));
    }
    let _ = events.send(WorkerEvent::Done(WorkerKind::Diarization));
}

/// Keeps the compiler honest about the engine abstraction in this module.
fn _assert_engine_is_object_safe(_: &dyn TranslationEngine) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::AsrFinal;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    /// A recognition double that is slower than the test and reports the
    /// flush it received.
    struct SlowRecognition {
        flushed: Arc<AtomicBool>,
        delay: Duration,
    }

    impl SlowRecognition {
        fn new(flushed: Arc<AtomicBool>) -> Self {
            Self {
                flushed,
                delay: Duration::from_millis(20),
            }
        }
    }

    impl Recognition for SlowRecognition {
        fn push(
            &mut self,
            _samples: &[f32],
            _start_frame_48k: u64,
            _emit: &mut dyn FnMut(AsrEvent),
        ) -> Result<(), String> {
            std::thread::sleep(self.delay);
            Ok(())
        }

        fn discontinuity(
            &mut self,
            _at_secs: f64,
            _emit: &mut dyn FnMut(AsrEvent),
        ) -> Result<(), String> {
            Ok(())
        }

        fn flush(&mut self, emit: &mut dyn FnMut(AsrEvent)) -> Result<(), String> {
            self.flushed.store(true, Ordering::SeqCst);
            emit(AsrEvent::Final(AsrFinal {
                local_id: 1,
                text: "last words".to_owned(),
                start_secs: 0.0,
                end_secs: 1.0,
                language: None,
                samples: Vec::new(),
            }));
            Ok(())
        }
    }

    #[test]
    fn the_stop_signal_reaches_a_worker_behind_a_full_audio_queue() {
        let flushed = Arc::new(AtomicBool::new(false));
        let (input_tx, input_rx) = bounded(8);
        let (control_tx, control_rx) = bounded(1);
        let (event_tx, event_rx) = bounded(64);
        let worker_flushed = flushed.clone();
        let handle = thread::spawn(move || {
            run_asr_worker(
                Box::new(SlowRecognition::new(worker_flushed)),
                input_rx,
                control_rx,
                event_tx,
            )
        });

        // The queue is full and the worker only drains one item at a time.
        for _ in 0..8 {
            input_tx
                .send(AsrInput::Audio {
                    samples: vec![0.0; 16],
                    start_frame_48k: 0,
                })
                .expect("the queue accepts the audio");
        }
        control_tx
            .send(AsrControl::Stop)
            .expect("the control channel accepts");
        drop(input_tx);

        handle.join().expect("the worker finishes");
        assert!(flushed.load(Ordering::SeqCst));

        let mut finals = 0;
        let mut done = false;
        while let Ok(event) = event_rx.recv_timeout(Duration::from_secs(2)) {
            match event {
                WorkerEvent::Asr(AsrEvent::Final(_)) => finals += 1,
                WorkerEvent::Done(WorkerKind::Asr) => done = true,
                _ => {}
            }
        }
        assert_eq!(finals, 1);
        assert!(done);
    }

    #[test]
    fn the_translation_queue_waits_for_the_last_recognition_event() {
        let (event_tx, event_rx) = bounded(4);
        let mut state = ShutdownState::new(false, false);
        state.worker_done(WorkerKind::Asr);

        // The last recognition result is still queued and not consumed.
        event_tx
            .send(WorkerEvent::Done(WorkerKind::Asr))
            .expect("the event queue accepts the event");
        assert!(!state.may_close_translation(&event_rx));

        // The queue may close once the session consumed the last event.
        event_rx.recv().expect("the event is pending");
        assert!(state.may_close_translation(&event_rx));
    }

    #[test]
    fn the_session_waits_for_every_worker() {
        let mut state = ShutdownState::new(false, false);
        state.worker_done(WorkerKind::Asr);
        assert!(!state.all_finished());
        state.worker_done(WorkerKind::Translation);
        assert!(state.all_finished());
    }

    #[test]
    fn a_disabled_worker_counts_as_finished() {
        let state = ShutdownState::new(false, false);
        assert!(state.speaker_done);
        assert!(state.diarization_done);
        assert!(!state.all_finished());
    }

    #[test]
    fn the_translation_queue_is_closed_only_once() {
        let (_event_tx, event_rx) = bounded(4);
        let mut state = ShutdownState::new(false, false);
        state.worker_done(WorkerKind::Asr);
        assert!(state.may_close_translation(&event_rx));
        state.mark_translation_closed();
        assert!(state.translation_closed());
        assert!(!state.may_close_translation(&event_rx));
    }

    /// A translation double that is slower than the test.
    struct SlowTranslation {
        delay: Duration,
    }

    impl TranslationEngine for SlowTranslation {
        fn capabilities(&self) -> crate::translation::TranslationCapabilities {
            crate::translation::TranslationCapabilities {
                engine: "test".to_owned(),
                runtime: "test".to_owned(),
                model_family: "test".to_owned(),
                mode: "text".to_owned(),
                streaming: false,
                reason: None,
            }
        }

        fn translate(&mut self, text: &str) -> Result<String, String> {
            std::thread::sleep(self.delay);
            Ok(format!("translated {text}"))
        }
    }

    #[test]
    fn a_slow_translation_worker_reports_its_last_translation_before_it_exits() {
        let queue = Arc::new(TranslationQueue::new());
        let (event_tx, event_rx) = bounded(8);
        let worker_queue = queue.clone();
        let handle = thread::spawn(move || {
            run_worker(
                Box::new(SlowTranslation {
                    delay: Duration::from_millis(30),
                }),
                &worker_queue,
                |result| {
                    let _ = event_tx.send(WorkerEvent::Translation(result));
                },
            );
            let _ = event_tx.send(WorkerEvent::Done(WorkerKind::Translation));
        });

        // The session queues a translation and stops while the worker is
        // still busy with it.
        assert!(matches!(
            queue.push(crate::translation::TranslationRequest {
                session_id: "session".to_owned(),
                segment_id: "segment".to_owned(),
                text_revision: 1,
                config_revision: 1,
                text: "hello".to_owned(),
                source_language: "en".to_owned(),
                target_language: "zh-Hans".to_owned(),
                model_id: "test".to_owned(),
            }),
            crate::translation::PushOutcome::Queued
        ));
        thread::sleep(Duration::from_millis(5));
        queue.close();

        let mut translated = 0;
        let mut done = false;
        while let Ok(event) = event_rx.recv_timeout(Duration::from_secs(2)) {
            match event {
                WorkerEvent::Translation(_) => translated += 1,
                WorkerEvent::Done(WorkerKind::Translation) => done = true,
                _ => {}
            }
        }
        assert_eq!(translated, 1);
        assert!(done);
        handle.join().expect("the translation worker finishes");
    }

    #[test]
    fn a_closed_translation_queue_releases_a_worker_without_pending_work() {
        let queue = Arc::new(TranslationQueue::new());
        let (event_tx, event_rx) = bounded(8);
        let worker_queue = queue.clone();
        let handle = thread::spawn(move || {
            run_worker(
                Box::new(SlowTranslation {
                    delay: Duration::from_millis(0),
                }),
                &worker_queue,
                |result| {
                    let _ = event_tx.send(WorkerEvent::Translation(result));
                },
            );
            let _ = event_tx.send(WorkerEvent::Done(WorkerKind::Translation));
        });

        // The session stopped before any request was queued. The worker must
        // still leave the queue instead of waiting for work forever.
        queue.close();
        let mut done = false;
        while let Ok(event) = event_rx.recv_timeout(Duration::from_secs(2)) {
            if matches!(event, WorkerEvent::Done(WorkerKind::Translation)) {
                done = true;
            }
        }
        assert!(done);
        handle.join().expect("the translation worker finishes");
    }

    #[test]
    fn a_failing_initialization_releases_a_worker_that_is_blocked_on_the_event_queue() {
        // A worker writes more events than the queue can hold. Without a
        // reader it would block forever, and the cleanup of a failed session
        // start could never join it.
        let (event_tx, event_rx) = bounded(1);
        let worker_tx = event_tx.clone();
        let released = Arc::new(AtomicBool::new(false));
        let worker_released = released.clone();
        let handle = thread::spawn(move || {
            for _ in 0..8 {
                let _ = worker_tx.send(WorkerEvent::Done(WorkerKind::Asr));
            }
            worker_released.store(true, Ordering::SeqCst);
        });
        drop(event_tx);

        let workers = WorkerRuntime {
            events: event_rx,
            asr_tx: None,
            asr_control_tx: None,
            speaker_tx: None,
            diarization_tx: None,
            queue: Arc::new(TranslationQueue::new()),
            handles: vec![(WorkerKind::Asr, handle)],
        };
        workers.shutdown();

        assert!(released.load(Ordering::SeqCst));
    }
}
