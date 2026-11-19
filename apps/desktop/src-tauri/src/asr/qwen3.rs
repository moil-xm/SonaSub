use sherpa_onnx::{OfflineQwen3ASRModelConfig, OfflineRecognizer, OfflineRecognizerConfig};

use crate::models::{ASR_MODELS, ASR_QWEN3_06B_ID};

use super::segmented::{RecognizerBackend, SegmentedConfig};

pub struct Qwen3Backend {
    recognizer: OfflineRecognizer,
}

impl Qwen3Backend {
    pub fn new(source: &SegmentedConfig) -> Result<Self, String> {
        let (Some(conv_frontend), Some(encoder), Some(decoder), Some(tokenizer)) = (
            &source.qwen3_conv_frontend,
            &source.qwen3_encoder,
            &source.qwen3_decoder,
            &source.qwen3_tokenizer,
        ) else {
            return Err("Qwen3 model paths are incomplete.".to_owned());
        };

        let mut config = OfflineRecognizerConfig::default();
        config.model_config.qwen3_asr = OfflineQwen3ASRModelConfig {
            conv_frontend: Some(conv_frontend.clone()),
            encoder: Some(encoder.clone()),
            decoder: Some(decoder.clone()),
            tokenizer: Some(tokenizer.clone()),
            ..OfflineQwen3ASRModelConfig::default()
        };
        config.model_config.tokens = Some(String::new());
        config.model_config.num_threads = 2;
        let recognizer = OfflineRecognizer::create(&config)
            .ok_or_else(|| "Failed to initialize Qwen3 ASR".to_owned())?;
        Ok(Self { recognizer })
    }
}

impl RecognizerBackend for Qwen3Backend {
    fn transcribe(&mut self, samples: &[f32]) -> Result<(String, Option<String>), String> {
        let stream = self.recognizer.create_stream();
        stream.accept_waveform(16_000, samples);
        self.recognizer.decode(&stream);
        let result = stream
            .get_result()
            .ok_or_else(|| "Qwen3 ASR returned no recognition result".to_owned())?;
        let (text, language) = parse_qwen3_text(&result.text);
        Ok((text, language.map(ToOwned::to_owned)))
    }
}

fn parse_qwen3_text(text: &str) -> (String, Option<&'static str>) {
    let mut text = text.trim();
    let mut language = None;
    loop {
        if let Some(remainder) = text.strip_prefix("<asr_text>") {
            text = remainder.trim_start();
            continue;
        }
        let is_language_scaffold = text.starts_with("language ")
            && text.find("<asr_text>").is_some_and(|marker| marker <= 64);
        if !is_language_scaffold {
            break;
        }
        if let Some((_, remainder)) = text.split_once("<asr_text>") {
            let label = text
                .strip_prefix("language ")
                .and_then(|value| value.split_once("<asr_text>"))
                .map(|(label, _)| label.trim());
            language = label.and_then(qwen3_language_code).or(language);
            text = remainder.trim_start();
            continue;
        }
        break;
    }
    (text.trim().to_owned(), language)
}

fn qwen3_language_code(label: &str) -> Option<&'static str> {
    let model = ASR_MODELS
        .iter()
        .find(|model| model.id == ASR_QWEN3_06B_ID)?;
    model
        .source_languages
        .iter()
        .copied()
        .find(|language| qwen3_language_name(language).eq_ignore_ascii_case(label))
}

fn qwen3_language_name(language: &str) -> &str {
    match language {
        "zh" => "Chinese",
        "en" => "English",
        "yue" => "Cantonese",
        "ar" => "Arabic",
        "de" => "German",
        "fr" => "French",
        "es" => "Spanish",
        "pt" => "Portuguese",
        "id" => "Indonesian",
        "it" => "Italian",
        "ko" => "Korean",
        "ru" => "Russian",
        "th" => "Thai",
        "vi" => "Vietnamese",
        "ja" => "Japanese",
        "tr" => "Turkish",
        "hi" => "Hindi",
        "ms" => "Malay",
        "nl" => "Dutch",
        "sv" => "Swedish",
        "da" => "Danish",
        "fi" => "Finnish",
        "pl" => "Polish",
        "cs" => "Czech",
        "fil" => "Filipino",
        "fa" => "Persian",
        "el" => "Greek",
        "hu" => "Hungarian",
        "mk" => "Macedonian",
        "ro" => "Romanian",
        language => language,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_qwen3_text, qwen3_language_name};
    use crate::models::{ASR_MODELS, ASR_QWEN3_06B_ID};

    #[test]
    fn removes_the_language_scaffold_from_qwen3_output() {
        assert_eq!(
            parse_qwen3_text("language English<asr_text>Clothes and lodging are free."),
            ("Clothes and lodging are free.".to_owned(), Some("en"))
        );
        assert_eq!(
            parse_qwen3_text("<asr_text> language Japanese<asr_text>こんにちは"),
            ("こんにちは".to_owned(), Some("ja"))
        );
        assert_eq!(
            parse_qwen3_text("language French<asr_text>Bonjour"),
            ("Bonjour".to_owned(), Some("fr"))
        );
    }

    #[test]
    fn keeps_regular_text_that_mentions_language_later() {
        assert_eq!(
            parse_qwen3_text("The language English is useful."),
            ("The language English is useful.".to_owned(), None)
        );
    }

    #[test]
    fn every_registry_language_has_a_qwen3_decoder_name() {
        let model = ASR_MODELS
            .iter()
            .find(|model| model.id == ASR_QWEN3_06B_ID)
            .expect("Qwen3 0.6B is registered");
        assert_eq!(model.source_languages.len(), 30);
        for language in model.source_languages {
            assert_ne!(
                qwen3_language_name(language),
                *language,
                "Qwen3 needs the decoder's English language name for `{language}`"
            );
        }
    }
}
