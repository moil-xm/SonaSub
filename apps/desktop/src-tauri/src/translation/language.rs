pub(crate) fn source_language(text: &str, language: Option<&str>) -> String {
    if let Some(language) = language {
        let normalized = normalize_language(language);
        if !normalized.is_empty() && normalized != "auto" {
            return normalized.to_owned();
        }
    }
    detect_translation_source(text).to_owned()
}

pub fn detect_translation_source(text: &str) -> &'static str {
    let mut han = 0usize;
    let mut latin = 0usize;
    for character in text.chars() {
        if ('\u{4e00}'..='\u{9fff}').contains(&character) {
            han += 1;
        } else if character.is_ascii_alphabetic() {
            latin += 1;
        }
    }
    if han > 0 && han * 2 >= latin {
        "zh-Hans"
    } else {
        "en"
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
    use super::source_language;

    #[test]
    fn translation_source_uses_the_written_script() {
        assert_eq!(source_language("今天天气很好。", None), "zh-Hans");
        assert_eq!(source_language("The weather is nice.", None), "en");
        assert_eq!(source_language("123", Some("<|zh|>")), "zh-Hans");
        assert_eq!(source_language("こんにちは", Some("ja")), "ja");
    }
}
