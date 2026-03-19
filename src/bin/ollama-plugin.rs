use std::env;
use std::io::{self, Read};
use std::process::ExitCode;

use serde::{Deserialize, Serialize};

const DEFAULT_OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";
const DEFAULT_OLLAMA_MODEL: &str = "gemma3:1b";

#[derive(Debug, Serialize, PartialEq, Eq)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct GenerateResponse {
    #[serde(default)]
    response: String,
    #[serde(default)]
    error: Option<String>,
}

// Plugboard's worker contract writes the claimed message body to this binary on stdin.
// This adapter reads that body once, then sends it to a local Ollama service as the
// prompt for /api/generate with stream=false. In other words, stdin is used at the
// Plugboard -> plugin boundary, while the plugin itself talks to an already-running
// local model backend over HTTP and still returns one bounded stdout result.
fn build_endpoint(base_url: &str) -> String {
    format!("{}/api/generate", base_url.trim_end_matches('/'))
}

fn build_request<'a>(model: &'a str, prompt: &'a str) -> GenerateRequest<'a> {
    GenerateRequest {
        model,
        prompt,
        stream: false,
    }
}

fn parse_response(body: &str) -> Result<GenerateResponse, serde_json::Error> {
    serde_json::from_str(body)
}

fn render_http_error(body: &str) -> String {
    match parse_response(body) {
        Ok(response) => response
            .error
            .filter(|message| !message.trim().is_empty())
            .unwrap_or_else(|| "ollama request failed".to_string()),
        Err(_) => {
            let body = body.trim();
            if body.is_empty() {
                "ollama request failed without output".to_string()
            } else {
                body.to_string()
            }
        }
    }
}

fn main() -> ExitCode {
    if let Err(error) = run() {
        eprintln!("{error}");
        return ExitCode::from(1);
    }

    ExitCode::SUCCESS
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut prompt = String::new();
    io::stdin().read_to_string(&mut prompt)?;

    let base_url =
        env::var("OLLAMA_PLUGIN_BASE_URL").unwrap_or_else(|_| DEFAULT_OLLAMA_BASE_URL.to_string());
    let model = select_model(
        env::var("PLUGBOARD_META_MODEL").ok().as_deref(),
        env::var("OLLAMA_PLUGIN_MODEL").ok().as_deref(),
    );
    let endpoint = build_endpoint(&base_url);
    let request = build_request(&model, &prompt);

    let response = ureq::post(&endpoint)
        .set("Content-Type", "application/json")
        .send_json(serde_json::to_value(&request)?);

    let body = match response {
        Ok(response) => response.into_string()?,
        Err(ureq::Error::Status(_, response)) => {
            return Err(render_http_error(&response.into_string()?).into());
        }
        Err(ureq::Error::Transport(error)) => return Err(error.to_string().into()),
    };
    let parsed = parse_response(&body)?;

    if let Some(error) = parsed.error.filter(|message| !message.trim().is_empty()) {
        return Err(error.into());
    }

    print!("{}", parsed.response);
    Ok(())
}

fn select_model(meta_model: Option<&str>, plugin_model: Option<&str>) -> String {
    meta_model
        .or(plugin_model)
        .unwrap_or(DEFAULT_OLLAMA_MODEL)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_OLLAMA_BASE_URL, DEFAULT_OLLAMA_MODEL, GenerateResponse, build_endpoint,
        build_request, parse_response, render_http_error, select_model,
    };

    #[test]
    fn appends_generate_path_to_base_url() {
        assert_eq!(
            build_endpoint(DEFAULT_OLLAMA_BASE_URL),
            "http://127.0.0.1:11434/api/generate"
        );
        assert_eq!(
            build_endpoint("http://localhost:11434/"),
            "http://localhost:11434/api/generate"
        );
    }

    #[test]
    fn builds_non_streaming_request() {
        assert_eq!(
            build_request(DEFAULT_OLLAMA_MODEL, "Explain this diff"),
            super::GenerateRequest {
                model: DEFAULT_OLLAMA_MODEL,
                prompt: "Explain this diff",
                stream: false,
            }
        );
    }

    #[test]
    fn parses_success_payload() {
        let payload = parse_response(r#"{ "response": "local reply", "done": true }"#).unwrap();
        assert_eq!(
            payload,
            GenerateResponse {
                response: "local reply".into(),
                error: None,
            }
        );
    }

    #[test]
    fn parses_error_payload() {
        let payload = parse_response(r#"{ "error": "model not found" }"#).unwrap();
        assert_eq!(
            payload,
            GenerateResponse {
                response: String::new(),
                error: Some("model not found".into()),
            }
        );
    }

    #[test]
    fn prefers_json_error_message() {
        assert_eq!(
            render_http_error(r#"{ "error": "model not found" }"#),
            "model not found"
        );
    }

    #[test]
    fn falls_back_to_plain_body_for_non_json_errors() {
        assert_eq!(
            render_http_error("service unavailable"),
            "service unavailable"
        );
    }

    #[test]
    fn prefers_plugboard_meta_model_over_env_override() {
        assert_eq!(
            select_model(Some("llama3.2:3b"), Some("gemma3:1b")),
            "llama3.2:3b"
        );
    }

    #[test]
    fn falls_back_from_plugin_env_to_default_model() {
        assert_eq!(select_model(None, Some("qwen2:1.5b")), "qwen2:1.5b");
        assert_eq!(select_model(None, None), DEFAULT_OLLAMA_MODEL);
    }
}
