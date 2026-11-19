pub(crate) fn source_language(text: &str, language: Option<&str>) -> String {
    if let Some(language) = language {
        let normalized = normalize_language(language);
        if !normalized.is_empty() && normalized != "auto" {
            return normalized.to_owned();
        }
    }
    detect_translation_source(text).to_owned()
}

/// Qwen3 ASR cleans its language hint out of the transcript. Without a
/// recognized tag, script detection will misroute Japanese to the English
/// translation model, so keep an explicit user selection for this adapter.
pub(crate) fn prompt_language(language: Option<&str>) -> Option<&str> {
    let language = language?;
    let normalized = normalize_language(language);
    if normalized.is_empty() || normalized == "auto" {
        None
    } else {
        Some(normalized)
    }
}

pub fn detect_translation_source(text: &str) -> &'static str {
    let mut han = 0usize;
    let mut latin = 0usize;
    let mut kana = 0usize;
    let mut hangul = 0usize;
    let mut other_alphabetic = 0usize;
    for character in text.chars() {
        if ('\u{4e00}'..='\u{9fff}').contains(&character) {
            han += 1;
        } else if character.is_ascii_alphabetic() {
            latin += 1;
        } else if ('\u{3040}'..='\u{30ff}').contains(&character) {
            kana += 1;
        } else if ('\u{ac00}'..='\u{d7af}').contains(&character)
            || ('\u{1100}'..='\u{11ff}').contains(&character)
        {
            hangul += 1;
        } else if character.is_alphabetic() {
            other_alphabetic += 1;
        }
    }
    if kana > 0 {
        "ja"
    } else if hangul > 0 {
        "ko"
    } else if other_alphabetic > 0 {
        "unknown"
    } else if han > 0 && han * 2 >= latin {
        "zh-Hans"
    } else if latin > 0 {
        "en"
    } else {
        "unknown"
    }
}

fn normalize_language(language: &str) -> &str {
    let language = language
        .trim()
        .trim_start_matches("<|")
        .trim_end_matches("|>");
    if language.starts_with("zh") || language == "yue" {
        "zh-Hans"
    } else if language.starts_with("en") {
        "en"
    } else {
        language
    }
}

#[cfg(test)]
mod tests {
    use super::{prompt_language, source_language};

    #[test]
    fn translation_source_uses_the_written_script() {
        assert_eq!(source_language("今天天气很好。", None), "zh-Hans");
        assert_eq!(source_language("The weather is nice.", None), "en");
        assert_eq!(source_language("こんにちは世界", None), "ja");
        assert_eq!(source_language("안녕하세요", None), "ko");
        assert_eq!(source_language("สวัสดี", None), "unknown");
        assert_eq!(source_language("123", None), "unknown");
        assert_eq!(source_language("123", Some("<|zh|>")), "zh-Hans");
        assert_eq!(source_language("こんにちは", Some("ja")), "ja");
    }

    #[test]
    fn explicit_language_survives_qwen3_text_cleaning() {
        assert_eq!(prompt_language(Some("ja")), Some("ja"));
        assert_eq!(prompt_language(Some("auto")), None);
    }
}
