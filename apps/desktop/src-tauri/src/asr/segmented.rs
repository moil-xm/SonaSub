use sherpa_rs::{
    sense_voice::{SenseVoiceConfig, SenseVoiceRecognizer},
    silero_vad::{SileroVad, SileroVadConfig},
};

use crate::asr::{AsrEvent, AsrFinal};

pub struct SegmentedConfig {
    pub vad_model: String,
    pub asr_model: String,
    pub tokens: String,
    pub language: String,
    pub max_segment_secs: f32,
    pub vad_threshold: f32,
    pub min_silence_secs: f32,
    pub min_speech_secs: f32,
}

pub struct SegmentedEngine {
    vad: SileroVad,
    recognizer: SenseVoiceRecognizer,
    epoch_secs: f64,
    fed_any: bool,
    next_local_id: u64,
    max_segment_secs: f32,
}

impl SegmentedEngine {
    pub fn new(config: SegmentedConfig) -> Result<Self, String> {
        let vad = SileroVad::new(
            SileroVadConfig {
                model: config.vad_model,
                min_silence_duration: config.min_silence_secs,
                min_speech_duration: config.min_speech_secs,
                max_speech_duration: config.max_segment_secs,
                threshold: config.vad_threshold,
                sample_rate: 16_000,
                window_size: 512,
                provider: Some("cpu".to_owned()),
                num_threads: Some(1),
                debug: false,
            },
            90.0,
        )
        .map_err(|error| format!("Failed to initialize Silero VAD: {error}"))?;
        let recognizer = SenseVoiceRecognizer::new(SenseVoiceConfig {
            model: config.asr_model,
            language: config.language,
            use_itn: true,
            provider: Some("cpu".to_owned()),
            num_threads: Some(2),
            debug: false,
            tokens: config.tokens,
        })
        .map_err(|error| format!("Failed to initialize SenseVoice: {error}"))?;
        Ok(Self {
            vad,
            recognizer,
            epoch_secs: 0.0,
            fed_any: false,
            next_local_id: 1,
            max_segment_secs: config.max_segment_secs,
        })
    }

    pub fn push(
        &mut self,
        samples: &[f32],
        start_frame_48k: u64,
        emit: &mut dyn FnMut(AsrEvent),
    ) -> Result<(), String> {
        if !self.fed_any {
            self.epoch_secs = start_frame_48k as f64 / 48_000.0;
            self.fed_any = true;
        }
        self.vad.accept_waveform(samples.to_vec());
        self.drain(emit);
        Ok(())
    }

    pub fn discontinuity(&mut self, at_secs: f64, emit: &mut dyn FnMut(AsrEvent)) {
        self.vad.flush();
        self.drain(emit);
        self.vad.clear();
        self.epoch_secs = at_secs;
        self.fed_any = false;
    }

    pub fn flush(&mut self, emit: &mut dyn FnMut(AsrEvent)) -> Result<(), String> {
        self.vad.flush();
        self.drain(emit);
        Ok(())
    }

    fn drain(&mut self, emit: &mut dyn FnMut(AsrEvent)) {
        while !self.vad.is_empty() {
            let segment = self.vad.front();
            self.vad.pop();
            let start_secs = self.epoch_secs + segment.start as f64 / 16_000.0;
            let end_secs = start_secs + segment.samples.len() as f64 / 16_000.0;
            if segment.samples.len() < (0.1 * 16_000.0) as usize {
                continue;
            }
            let result = self.recognizer.transcribe(16_000, &segment.samples);
            let text = sanitize_sense_voice_text(&result.text);
            if text.is_empty() {
                continue;
            }
            let language = if sanitize_language(&result.lang).is_empty() {
                None
            } else {
                Some(sanitize_language(&result.lang))
            };
            let local_id = self.next_local_id;
            self.next_local_id += 1;
            emit(AsrEvent::Final(AsrFinal {
                local_id,
                text,
                start_secs,
                end_secs: end_secs.min(start_secs + self.max_segment_secs as f64 + 0.5),
                language,
                samples: segment.samples,
            }));
        }
    }
}

fn sanitize_sense_voice_text(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut in_tag = false;
    for character in text.chars() {
        if character == '<' {
            in_tag = true;
            continue;
        }
        if in_tag {
            if character == '>' {
                in_tag = false;
            }
            continue;
        }
        output.push(character);
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sanitize_language(language: &str) -> String {
    language
        .trim()
        .trim_start_matches("<|")
        .trim_end_matches("|>")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_sense_voice_metadata_tags() {
        assert_eq!(
            sanitize_sense_voice_text("<|en|><|NEUTRAL|> Hello   world <|Speech|>"),
            "Hello world"
        );
    }

    #[test]
    fn strips_angle_brackets_from_language_metadata() {
        assert_eq!(sanitize_language("<|zh|>"), "zh");
    }
}
