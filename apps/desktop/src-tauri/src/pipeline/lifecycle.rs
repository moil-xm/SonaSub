use std::thread::JoinHandle;

use crossbeam_channel::{bounded, Receiver, Sender};

use super::workers::{DiarizationInput, SpeakerInput, WorkerRuntime};
use crate::{asr::AsrControl, audio::CaptureEvent};

pub(crate) fn stop_capture(
    capture_handle: &mut Option<JoinHandle<()>>,
    capture_rx: &mut Receiver<CaptureEvent>,
    keepalive: &mut Option<Sender<CaptureEvent>>,
) {
    // Keep the receiver alive after capture stops so the runner can still
    // select on its event channels while inference finishes. The sender is
    // intentionally retained even though no further audio will arrive.
    if let Some(handle) = capture_handle.take() {
        let _ = handle.join();
    }
    let (sender, receiver) = bounded(1);
    *capture_rx = receiver;
    *keepalive = Some(sender);
}

pub(crate) fn stop_workers(workers: &mut WorkerRuntime) {
    // The recognition stop signal travels on the control channel so it is
    // never blocked behind a full audio backlog. A busy recognition worker
    // therefore always reaches its flush and reports the last subtitle.
    if let Some(sender) = workers.asr_control_tx.as_ref() {
        if let Err(error) = sender.try_send(AsrControl::Stop) {
            if error.is_disconnected() {
                workers.asr_control_tx = None;
            }
        }
    }
    if let Some(sender) = workers.speaker_tx.as_ref() {
        if sender.try_send(SpeakerInput::Stop).is_err() {
            workers.speaker_tx = None;
        }
    }
    if let Some(sender) = workers.diarization_tx.as_ref() {
        if sender.try_send(DiarizationInput::Stop).is_err() {
            workers.diarization_tx = None;
        }
    }
}
