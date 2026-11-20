use std::{
    mem,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crossbeam_channel::{bounded, tick, Receiver, Sender, TrySendError};

use crate::{
    asr::{AsrEngine, AsrEvent, AsrFinal, AsrInput, AsrPartial},
    audio::{self, AudioChunk, AudioSourceKind, CaptureEvent, CaptureOptions},
    config::{AppConfig, TranslationTrigger},
    models::{resolve_asr_mode, ModelPaths, ASR_MODELS, DIARIZATION_FILES, SPEAKER_FILES},
    pipeline::{
        audio_flow::AudioFlow,
        lifecycle,
        metrics::{SessionMetrics, SessionStats},
        workers::{
            DiarizationInput, DiarizationResult, DiarizationTask, ShutdownState, SpeakerInput,
            SpeakerResult, SpeakerTask, WorkerEvent, WorkerKind, WorkerRuntime,
        },
    },
    speaker::{DiarizationWindow, SpeakerEngine, SpeakerRegistry},
    subtitles::{SubtitlePatch, SubtitleStore},
    translation::{
        prompt_language, resolve_translation_config, resolve_translation_route, source_language,
        PushOutcome, TranslationRequest, TranslationResult, TranslationRoute,
    },
};

use super::types::{RecordNotice, RecordStats, SourceEndReason};

const UI_BATCH_INTERVAL: Duration = Duration::from_millis(80);
const STATS_INTERVAL: Duration = Duration::from_millis(750);
const NATIVE_WAIT_NOTICE_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct SourceContext {
    pub record_id: String,
    pub source_run_id: String,
    pub source_kind: AudioSourceKind,
    pub device_id: String,
    pub device_name: String,
    pub config: AppConfig,
    pub models: ModelPaths,
    pub elapsed_base_ms: u64,
    pub occurrence_base_ms: u64,
    pub history_limit: usize,
    pub stop: Arc<AtomicBool>,
    pub completion: mpsc::Sender<()>,
    pub events: Sender<SourceEvent>,
}

#[derive(Debug)]
pub enum SourceEvent {
    Subtitle {
        record_id: String,
        source_run_id: String,
        patches: Vec<SubtitlePatch>,
    },
    Stats {
        record_id: String,
        source_run_id: String,
        stats: RecordStats,
    },
    Notice {
        record_id: String,
        source_run_id: String,
        notice: RecordNotice,
    },
    Finished {
        record_id: String,
        source_run_id: String,
        outcome: SourceOutcome,
        ack: mpsc::Sender<()>,
    },
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct SourceOutcome {
    pub captured_ms: u64,
    pub failure: Option<String>,
    pub reason: SourceEndReason,
}

pub fn spawn(
    context: SourceContext,
    ready: mpsc::Sender<Result<(), String>>,
) -> Result<JoinHandle<()>, String> {
    thread::Builder::new()
        .name(format!("sonasub-source-{}", context.device_name))
        .spawn(move || run_source(context, ready))
        .map_err(|error| format!("Failed to start the source thread: {error}"))
}

fn run_source(context: SourceContext, ready: mpsc::Sender<Result<(), String>>) {
    match SourceRunner::initialize(context.clone()) {
        Ok(runner) => {
            if ready.send(Ok(())).is_err() {
                context.stop.store(true, Ordering::SeqCst);
            }
            let outcome = runner.run();
            let _ = context.events.send(SourceEvent::Finished {
                record_id: context.record_id,
                source_run_id: context.source_run_id,
                outcome,
                ack: context.completion,
            });
        }
        Err(error) => {
            let _ = ready.send(Err(error));
        }
    }
}

struct SourceRunner {
    context: SourceContext,
    capture_rx: Receiver<CaptureEvent>,
    _capture_keepalive: Option<Sender<CaptureEvent>>,
    capture_handle: Option<JoinHandle<()>>,
    workers: WorkerRuntime,
    store: SubtitleStore,
    audio: AudioFlow,
    pending_patches: Vec<SubtitlePatch>,
    last_patch_emit: Instant,
    last_stats_emit: Instant,
    last_diarization: Instant,
    shutdown_started: Option<Instant>,
    last_wait_notice: Instant,
    ending: bool,
    failure: Option<String>,
    end_reason: SourceEndReason,
    shutdown: ShutdownState,
    metrics: SessionMetrics,
}

impl SourceRunner {
    fn initialize(mut context: SourceContext) -> Result<Self, String> {
        let selected_asr = ASR_MODELS
            .iter()
            .find(|model| model.id == context.config.recognition.model_id)
            .ok_or_else(|| {
                format!(
                    "Unknown ASR model `{}`.",
                    context.config.recognition.model_id
                )
            })?;
        let asr_mode = resolve_asr_mode(selected_asr, context.config.recognition.mode)?;
        let translation = resolve_translation_config(&context.config.translation)?;
        let missing = context.models.missing(selected_asr.files);
        if !missing.is_empty() {
            return Err(format!(
                "Missing ASR model files in {}: {}.",
                context.models.root.display(),
                missing.join(", ")
            ));
        }
        if let Some(translation_spec) = translation.bergamot_spec {
            let missing = context
                .models
                .missing(&crate::models::translation_files(translation_spec));
            if !missing.is_empty() {
                return Err(format!(
                    "Missing translation model files in {}: {}.",
                    context.models.root.display(),
                    missing.join(", ")
                ));
            }
        }
        let speakers_enabled = context.config.speakers.enabled;
        let diarization_enabled = speakers_enabled && context.config.speakers.diarization_enabled;
        if speakers_enabled {
            let missing = context.models.missing(SPEAKER_FILES);
            if !missing.is_empty() {
                return Err(format!(
                    "Missing speaker embedding files in {}: {}.",
                    context.models.root.display(),
                    missing.join(", ")
                ));
            }
        }
        if diarization_enabled {
            let missing = context.models.missing(DIARIZATION_FILES);
            if !missing.is_empty() {
                return Err(format!(
                    "Missing diarization files in {}: {}.",
                    context.models.root.display(),
                    missing.join(", ")
                ));
            }
        }

        let asr_engine = AsrEngine::load(
            asr_mode,
            &context.models,
            &context.config.recognition,
            selected_asr.id,
        )?;
        let mut speaker_engine = None;
        let mut diarization_engine = None;
        let mut registry = None;
        if speakers_enabled {
            let model = context.models.path("sherpa/speaker/campplus-zh-en.onnx");
            speaker_engine = Some(SpeakerEngine::new(&model)?);
            registry = Some(Arc::new(Mutex::new(SpeakerRegistry::new(
                context.config.speakers.max_speakers,
                context.config.speakers.similarity_threshold,
            ))));
        }
        if diarization_enabled {
            let segmentation = context
                .models
                .path("sherpa/diarization/pyannote-segmentation-3.0.onnx");
            let embedding = context.models.path("sherpa/speaker/campplus-zh-en.onnx");
            diarization_engine = Some(crate::speaker::DiarizationEngine::new(
                &segmentation,
                &embedding,
            )?);
        }
        let workers = WorkerRuntime::spawn(
            Box::new(asr_engine),
            context.models.clone(),
            context.config.translation.clone(),
            translation.bergamot_spec,
            speaker_engine,
            diarization_engine,
            registry,
        )?;
        let (capture_tx, capture_rx) = bounded(250);
        let capture = match audio::start(
            CaptureOptions {
                kind: context.source_kind,
                device_id: Some(context.device_id.clone()),
            },
            capture_tx,
            context.stop.clone(),
        ) {
            Ok(capture) => capture,
            Err(error) => {
                workers.shutdown();
                return Err(error);
            }
        };
        let capture_info = capture.info;
        let capture_handle = capture.thread;
        if context.stop.load(Ordering::SeqCst) {
            let _ = capture_handle.join();
            workers.shutdown();
            return Err("Source start was cancelled.".to_owned());
        }
        let audio = match AudioFlow::new(
            f64::from(context.config.speakers.window_secs.max(30.0)) + 2.0,
            capture_info.sample_rate,
            capture_info.channels,
        ) {
            Ok(audio) => audio,
            Err(error) => {
                context.stop.store(true, Ordering::SeqCst);
                let _ = capture_handle.join();
                workers.shutdown();
                return Err(error);
            }
        };
        // Model loading happens before capture starts. Anchor record ordering
        // to the moment audio actually begins, not to the spawn request.
        context.occurrence_base_ms = now_ms();
        let now = Instant::now();
        Ok(Self {
            context: context.clone(),
            capture_rx,
            _capture_keepalive: None,
            capture_handle: Some(capture_handle),
            workers,
            store: SubtitleStore::for_source(
                context.record_id.clone(),
                context.source_run_id.clone(),
                context.source_kind,
                context.device_id.clone(),
                context.device_name.clone(),
                context.elapsed_base_ms,
                context.occurrence_base_ms,
                context.history_limit,
            ),
            audio,
            pending_patches: Vec::new(),
            last_patch_emit: now,
            last_stats_emit: now,
            last_diarization: now,
            shutdown_started: None,
            last_wait_notice: now,
            ending: false,
            failure: None,
            end_reason: SourceEndReason::RecordStopped,
            shutdown: ShutdownState::new(speakers_enabled, diarization_enabled),
            metrics: SessionMetrics::new(context.occurrence_base_ms),
        })
    }

    fn run(mut self) -> SourceOutcome {
        let ticker = tick(Duration::from_millis(50));
        loop {
            crossbeam_channel::select! {
                recv(self.capture_rx) -> message => {
                    match message {
                        Ok(event) => self.handle_capture(event),
                        Err(_) => {
                            if !self.ending {
                                if self.context.stop.load(Ordering::SeqCst) {
                                    self.begin_shutdown(None, SourceEndReason::RecordStopped);
                                } else {
                                    self.begin_shutdown(
                                        Some(format!(
                                            "{} capture ended unexpectedly.",
                                            self.context.source_kind.label()
                                        )),
                                        SourceEndReason::PipelineFailure,
                                    );
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
                                self.failure =
                                    Some("All native workers exited before the source stopped.".to_owned());
                                self.end_reason = SourceEndReason::PipelineFailure;
                                self.ending = true;
                            }
                            break;
                        }
                    }
                }
                recv(ticker) -> _ => self.handle_tick(),
            }
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
        let workers = mem::replace(&mut self.workers, WorkerRuntime::empty());
        workers.join();
        SourceOutcome {
            captured_ms: (self.audio.last_audio_secs() * 1_000.0).max(0.0) as u64,
            failure: self.failure,
            reason: self.end_reason,
        }
    }

    fn handle_capture(&mut self, event: CaptureEvent) {
        match event {
            CaptureEvent::Chunk(chunk) => {
                if !self.ending {
                    self.process_chunk(chunk);
                }
            }
            CaptureEvent::Warning(message) => self.notice("warning", message),
            CaptureEvent::DeviceLost(message) => {
                self.begin_shutdown(Some(message), SourceEndReason::DeviceLost);
            }
            CaptureEvent::Fatal(message) => {
                self.begin_shutdown(Some(message), SourceEndReason::PipelineFailure);
            }
        }
    }

    fn process_chunk(&mut self, chunk: AudioChunk) {
        let outcome = self.audio.process_chunk(&chunk);
        if outcome.discontinuity {
            self.last_diarization = Instant::now();
            self.metrics.add_audio(0, outcome.dropped_frames);
            if outcome.notify_gap {
                self.notice(
                    "warning",
                    format!(
                        "Audio gap detected at {:.2}s; subtitle timing continues after the real gap.",
                        outcome.start_secs
                    ),
                );
            }
        }
        self.metrics.add_audio(outcome.native_frames, 0);
        self.metrics.set_level(chunk.peak, chunk.rms);
        let asr_frames = outcome.samples.len() as u64;
        self.send_asr(
            AsrInput::Audio {
                samples: outcome.samples,
                start_frame_16k: outcome.start_frame_16k,
            },
            outcome.start_secs,
            asr_frames,
        );
    }

    fn send_asr(&mut self, input: AsrInput, start_secs: f64, asr_frames: u64) {
        if let Some(at_secs) = self.audio.pending_discontinuity() {
            let Some(sender) = self.workers.asr_tx.as_ref() else {
                self.begin_shutdown(
                    Some("The ASR worker stopped unexpectedly.".to_owned()),
                    SourceEndReason::PipelineFailure,
                );
                return;
            };
            match sender.try_send(AsrInput::Discontinuity { at_secs }) {
                Ok(()) => self.audio.clear_pending_discontinuity(),
                Err(TrySendError::Full(_)) => {
                    self.drop_asr_input(start_secs, asr_frames);
                    return;
                }
                Err(TrySendError::Disconnected(_)) => {
                    self.begin_shutdown(
                        Some("The ASR worker stopped unexpectedly.".to_owned()),
                        SourceEndReason::PipelineFailure,
                    );
                    return;
                }
            }
        }
        let Some(sender) = self.workers.asr_tx.as_ref() else {
            self.begin_shutdown(
                Some("The ASR worker stopped unexpectedly.".to_owned()),
                SourceEndReason::PipelineFailure,
            );
            return;
        };
        match sender.try_send(input) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => self.drop_asr_input(start_secs, asr_frames),
            Err(TrySendError::Disconnected(_)) => {
                self.begin_shutdown(
                    Some("The ASR worker stopped unexpectedly.".to_owned()),
                    SourceEndReason::PipelineFailure,
                );
            }
        }
    }

    fn drop_asr_input(&mut self, start_secs: f64, asr_frames: u64) {
        self.audio.mark_asr_dropped(start_secs);
        self.metrics.add_audio(0, asr_frames);
        self.notice(
            "warning",
            format!(
                "ASR could not keep up with {} audio. The dropped interval is excluded from the subtitle timeline.",
                self.context.source_kind.label().to_lowercase()
            ),
        );
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
            WorkerKind::Speaker => self.workers.speaker_tx = None,
            WorkerKind::Diarization => self.workers.diarization_tx = None,
            WorkerKind::Translation => {}
        }
        self.shutdown.worker_done(kind);
    }

    fn handle_asr_event(&mut self, event: AsrEvent) {
        match event {
            AsrEvent::Partial(partial) => self.apply_partial(partial),
            AsrEvent::Final(final_event) => self.apply_final(final_event),
            AsrEvent::Warning(message) => self.notice("warning", message),
            AsrEvent::Fatal(message) => {
                self.begin_shutdown(Some(message), SourceEndReason::PipelineFailure);
            }
        }
    }

    fn apply_partial(&mut self, partial: AsrPartial) {
        self.metrics.partial();
        let patches = self.store.apply_partial(partial);
        self.pending_patches.extend(patches);
    }

    fn apply_final(&mut self, final_event: AsrFinal) {
        let segment_id = format!("{}:{}", self.context.source_run_id, final_event.local_id);
        let samples = Arc::new(final_event.samples.clone());
        let latency_ms =
            ((self.audio.last_audio_secs() - final_event.end_secs).max(0.0) * 1_000.0) as u64;
        self.metrics.final_subtitle(latency_ms);
        let patches = self.store.apply_final(final_event.clone());
        self.pending_patches.extend(patches);
        if !self.ending && self.context.config.speakers.enabled && samples.len() >= 4_000 {
            let task = SpeakerTask {
                segment_id: segment_id.clone(),
                samples,
                queued_at: Instant::now(),
            };
            if let Some(sender) = self.workers.speaker_tx.as_ref() {
                match sender.try_send(SpeakerInput::Task(task)) {
                    Ok(()) => {}
                    Err(TrySendError::Full(_)) => self.notice(
                        "warning",
                        "Speaker analysis is behind; one final subtitle is marked as an unknown speaker."
                            .to_owned(),
                    ),
                    Err(TrySendError::Disconnected(_)) => self.notice(
                        "warning",
                        "The speaker worker stopped; speaker labels are unavailable.".to_owned(),
                    ),
                }
            }
        }
        self.queue_translation(&segment_id, final_event.text, &final_event.language);
    }

    fn handle_speaker_result(&mut self, result: SpeakerResult) {
        match result.result {
            Ok(Some(speaker)) => {
                self.metrics.speaker(result.latency_ms);
                if let Some(patch) = self.store.set_speaker(&result.segment_id, speaker) {
                    self.pending_patches.push(patch);
                }
            }
            Ok(None) => self.metrics.speaker(result.latency_ms),
            Err(error) => self.notice("warning", error),
        }
    }

    fn handle_diarization_result(&mut self, result: DiarizationResult) {
        match result.result {
            Ok(window) => self.apply_diarization(window),
            Err(error) => self.notice("warning", error),
        }
    }

    fn apply_diarization(&mut self, window: DiarizationWindow) {
        let segments = self.store.snapshot();
        let mut patches = Vec::new();
        for segment in &segments {
            let Some(speaker) = window.speaker_for_segment(segment.start_secs, segment.end_secs)
            else {
                continue;
            };
            if let Some(patch) = self.store.set_speaker(&segment.id, speaker) {
                patches.push(patch);
            }
        }
        self.pending_patches.extend(patches);
        self.metrics.diarization(window.latency_ms);
    }

    fn handle_translation_result(&mut self, result: TranslationResult) {
        if self.store.is_stale(&result.request, self.config_revision()) {
            return;
        }
        let failed = result.result.is_err();
        self.metrics.translation(result.latency_ms, failed);
        if let Some(patch) = self.store.set_translation(
            &result.request.segment_id,
            result.request.text_revision,
            result.result,
            &result.request.target_language,
            &result.request.engine,
            &result.request.model_id,
        ) {
            self.pending_patches.push(patch);
        }
    }

    fn queue_translation(&mut self, segment_id: &str, text: String, language: &Option<String>) {
        let target = crate::config::normalize_target_language(
            &self.context.config.translation.target_language,
        )
        .to_owned();
        if !self.store.needs_translation(segment_id, &target) {
            return;
        }
        let Some(text_revision) = self
            .store
            .segment(segment_id)
            .map(|segment| segment.revision)
        else {
            return;
        };
        let configured_language = if self.context.config.recognition.source_language == "auto" {
            None
        } else {
            prompt_language(Some(&self.context.config.recognition.source_language))
        };
        let source_language = source_language(&text, configured_language.or(language.as_deref()));
        let engine = self.context.config.translation.engine.clone();
        let route = resolve_translation_route(&self.context.config.translation, &source_language);
        let route = match route {
            Ok(route) => route,
            Err(error) => TranslationRoute::Unavailable(error),
        };
        let (engine, model_id) = match route {
            TranslationRoute::NotNeeded => {
                if let Some(patch) = self.store.mark_same_language(segment_id, &target, &engine) {
                    self.pending_patches.push(patch);
                }
                return;
            }
            TranslationRoute::Unavailable(error) => {
                if let Some(patch) = self
                    .store
                    .set_translation_unavailable(segment_id, error, &target, &engine)
                {
                    self.pending_patches.push(patch);
                }
                return;
            }
            TranslationRoute::Ready { engine, model_id } => (engine.to_owned(), model_id),
        };
        let request = TranslationRequest {
            session_id: self.context.source_run_id.clone(),
            segment_id: segment_id.to_owned(),
            text_revision,
            config_revision: self.config_revision(),
            text,
            source_language: source_language.to_owned(),
            target_language: target.clone(),
            engine: engine.clone(),
            model_id: model_id.clone(),
        };
        if let Some(patch) = self
            .store
            .mark_translation_pending(segment_id, &target, &engine, "text", &model_id)
        {
            self.pending_patches.push(patch);
        }
        match self.workers.queue.push(request) {
            PushOutcome::Queued => {}
            PushOutcome::Displaced(evicted) => {
                if let Some(patch) = self.store.set_translation_error(
                    &evicted.segment_id,
                    evicted.text_revision,
                    &evicted.target_language,
                    &evicted.engine,
                    &evicted.model_id,
                    "Translation queue was full; a newer subtitle superseded this request."
                        .to_owned(),
                ) {
                    self.pending_patches.push(patch);
                }
            }
            PushOutcome::Closed(rejected) => {
                if let Some(patch) = self.store.set_translation_unavailable(
                    &rejected.segment_id,
                    "The translation worker already stopped, so this subtitle stays source-only."
                        .to_owned(),
                    &rejected.target_language,
                    &rejected.engine,
                ) {
                    self.pending_patches.push(patch);
                }
            }
        }
    }

    fn handle_tick(&mut self) {
        for kind in self.workers.finished_workers() {
            if !self.ending && self.workers.events.is_empty() {
                self.failure = Some(format!("The {} worker exited unexpectedly.", kind.label()));
                self.end_reason = SourceEndReason::PipelineFailure;
                self.ending = true;
            }
            self.mark_worker_done(kind);
        }
        if self.context.stop.load(Ordering::SeqCst) && !self.ending {
            self.begin_shutdown(None, SourceEndReason::RecordStopped);
        }
        self.report_native_wait();
        if !self.ending {
            self.schedule_stable_translations();
            self.schedule_diarization();
        }
        self.flush_patches(false);
        if self.last_stats_emit.elapsed() >= STATS_INTERVAL {
            self.publish_stats();
        }
    }

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
        self.notice(
            "info",
            "Still waiting for the native speech runtime to return; the source keeps its worker until it finishes."
                .to_owned(),
        );
    }

    fn schedule_stable_translations(&mut self) {
        if self.context.config.translation.trigger != TranslationTrigger::StableSegment {
            return;
        }
        let candidates = self
            .store
            .stable_candidates(self.context.config.recognition.stable_wait_ms);
        for candidate in candidates {
            if !self.ending {
                self.queue_translation(
                    &candidate.segment_id,
                    candidate.text,
                    &candidate.source_language,
                );
            }
        }
    }

    fn schedule_diarization(&mut self) {
        if !self.context.config.speakers.enabled
            || !self.context.config.speakers.diarization_enabled
        {
            return;
        }
        let window_secs = self.context.config.speakers.window_secs as f64;
        let interval = Duration::from_secs_f64((window_secs / 2.0).max(5.0));
        if self.last_diarization.elapsed() < interval
            || self.audio.available_secs() + 0.001 < window_secs
        {
            return;
        }
        let Some((start_secs, samples)) = self.audio.snapshot(window_secs) else {
            return;
        };
        let Some(sender) = self.workers.diarization_tx.as_ref() else {
            return;
        };
        match sender.try_send(DiarizationInput::Task(DiarizationTask {
            start_secs,
            samples: Arc::new(samples),
        })) {
            Ok(()) => self.last_diarization = Instant::now(),
            Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => self.workers.diarization_tx = None,
        }
    }

    fn begin_shutdown(&mut self, failure: Option<String>, reason: SourceEndReason) {
        if self.ending {
            return;
        }
        self.ending = true;
        self.failure = failure;
        self.end_reason = reason;
        self.shutdown_started = Some(Instant::now());
        self.context.stop.store(true, Ordering::SeqCst);
        let pending_capture = lifecycle::stop_capture(
            &mut self.capture_handle,
            &mut self.capture_rx,
            &mut self._capture_keepalive,
        );
        for event in pending_capture {
            if let CaptureEvent::Chunk(chunk) = event {
                self.process_chunk(chunk);
            }
        }
        let asr_tail = self.finish_audio();
        lifecycle::stop_workers(&mut self.workers, asr_tail);
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
        let _ = self.context.events.send(SourceEvent::Subtitle {
            record_id: self.context.record_id.clone(),
            source_run_id: self.context.source_run_id.clone(),
            patches,
        });
        self.last_patch_emit = Instant::now();
    }

    fn publish_stats(&mut self) {
        let stats = record_stats(self.metrics.snapshot(), 1);
        let _ = self.context.events.send(SourceEvent::Stats {
            record_id: self.context.record_id.clone(),
            source_run_id: self.context.source_run_id.clone(),
            stats,
        });
        self.last_stats_emit = Instant::now();
    }

    fn notice(&self, level: &str, message: String) {
        let _ = self.context.events.send(SourceEvent::Notice {
            record_id: self.context.record_id.clone(),
            source_run_id: self.context.source_run_id.clone(),
            notice: RecordNotice {
                level: level.to_owned(),
                message,
                at_ms: now_ms(),
            },
        });
    }

    fn config_revision(&self) -> u64 {
        1
    }
}

fn record_stats(stats: SessionStats, active_sources: usize) -> RecordStats {
    RecordStats {
        started_at_ms: stats.started_at_ms,
        active_sources,
        audio_frames: stats.audio_frames,
        dropped_frames: stats.dropped_frames,
        partial_updates: stats.partial_updates,
        finalized_subtitles: stats.finalized_subtitles,
        translation_requests: stats.translation_requests,
        translation_failures: stats.translation_failures,
        speaker_updates: stats.speaker_updates,
        diarization_windows: stats.diarization_windows,
        audio_peak: stats.audio_peak,
        audio_rms: stats.audio_rms,
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
