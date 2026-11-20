use std::time::Duration;

use serde_json::Value;

use crate::config::normalize_openai_base_url;

const REMOTE_TIMEOUT: Duration = Duration::from_secs(15);

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(REMOTE_TIMEOUT)
        .timeout_read(REMOTE_TIMEOUT)
        .timeout_write(REMOTE_TIMEOUT)
        .build()
}

fn request_json(
    base_url: &str,
    api_key: &str,
    path: &str,
    body: Option<&str>,
) -> Result<String, String> {
    let base_url = normalize_openai_base_url(base_url)?;
    let url = format!("{base_url}{path}");
    let request = match body {
        Some(body) => {
            let request = agent().post(&url).set("Content-Type", "application/json");
            if api_key.trim().is_empty() {
                request
            } else {
                request.set("Authorization", &format!("Bearer {api_key}"))
            }
            .send_string(body)
        }
        None => {
            let request = agent().get(&url);
            if api_key.trim().is_empty() {
                request
            } else {
                request.set("Authorization", &format!("Bearer {api_key}"))
            }
            .call()
        }
    }
    .map_err(|error| format!("Provider request failed: {error}"))?;
    request
        .into_string()
        .map_err(|error| format!("Failed to read the provider response: {error}"))
}

fn json_error(value: &Value) -> Option<String> {
    let error = value.get("error")?;
    match error {
        Value::String(message) => Some(message.clone()),
        Value::Object(object) => object
            .get("message")
            .or_else(|| object.get("message"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| Some(error.to_string())),
        _ => Some(error.to_string()),
    }
}

pub fn fetch_models(base_url: &str, api_key: &str) -> Result<Vec<String>, String> {
    let body = request_json(base_url, api_key, "/models", None)?;
    let value: Value = serde_json::from_str(body.trim())
        .map_err(|error| format!("Invalid model response: {error}"))?;
    if let Some(message) = json_error(&value) {
        return Err(format!("Provider returned an error: {message}"));
    }
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| "Model response has no data array.".to_owned())?;
    let mut models = Vec::new();
    for entry in data {
        if let Some(id) = entry.get("id").and_then(Value::as_str) {
            let id = id.trim();
            if !id.is_empty() && !models.contains(&id.to_owned()) {
                models.push(id.to_owned());
            }
        }
    }
    Ok(models)
}

pub fn test_provider(base_url: &str, api_key: &str, model: &str) -> Result<String, String> {
    let model = model.trim();
    if model.is_empty() {
        return Err("Choose a model to test.".to_owned());
    }
    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": "Reply with OK." }],
        "max_tokens": 8
    })
    .to_string();
    let body = request_json(base_url, api_key, "/chat/completions", Some(&body))?;
    let value: Value = serde_json::from_str(body.trim())
        .map_err(|error| format!("Invalid chat response: {error}"))?;
    if let Some(message) = json_error(&value) {
        return Err(format!("Provider returned an error: {message}"));
    }
    let content = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_owned();
    if content.is_empty() {
        return Err("Provider returned an empty response.".to_owned());
    }
    Ok(truncate(&content, 400))
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::{fetch_models, test_provider};

    fn spawn_stub(body: &'static str) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 4096];
            let read = stream.read(&mut buffer).unwrap();
            let mut data = buffer[..read].to_vec();
            let header_end = data
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|position| position + 4);
            if let Some(header_end) = header_end {
                let headers = String::from_utf8_lossy(&data[..header_end]).into_owned();
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                while data.len() < header_end + content_length {
                    let read = stream.read(&mut buffer).unwrap();
                    if read == 0 {
                        break;
                    }
                    data.extend_from_slice(&buffer[..read]);
                }
            }
            let request = String::from_utf8_lossy(&data).into_owned();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).unwrap();
            request
        });
        (format!("http://{address}/v1"), handle)
    }

    #[test]
    fn fetches_model_ids_from_an_openai_compatible_endpoint() {
        let (base_url, server) = spawn_stub(
            r#"{"object":"list","data":[{"id":"gpt-test"},{"id":"gpt-test"},{"id":""}]}"#,
        );

        let models = fetch_models(&base_url, "").unwrap();

        assert_eq!(models, vec!["gpt-test"]);
        let request = server.join().unwrap();
        assert!(request.contains("GET /v1/models"), "request: {request:?}");
    }

    #[test]
    fn tests_a_selected_model_through_chat_completions() {
        let (base_url, server) =
            spawn_stub(r#"{"choices":[{"message":{"role":"assistant","content":"OK"}}]}"#);

        let reply = test_provider(&base_url, "", "gpt-test").unwrap();

        assert_eq!(reply, "OK");
        let request = server.join().unwrap();
        assert!(
            request.contains("POST /v1/chat/completions"),
            "request: {request}"
        );
        assert!(request.contains("gpt-test"), "request: {request:?}");
    }
}
