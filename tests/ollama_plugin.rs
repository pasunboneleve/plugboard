use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

fn spawn_fake_ollama(status_line: &str, response_body: &str) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let status_line = status_line.to_string();
    let response_body = response_body.to_string();
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 1024];
        let mut header_end = None;

        while header_end.is_none() {
            let read = stream.read(&mut chunk).unwrap();
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            header_end = buffer.windows(4).position(|window| window == b"\r\n\r\n");
        }

        let header_end = header_end.expect("missing header terminator");
        let header_bytes = &buffer[..header_end + 4];
        let headers = String::from_utf8_lossy(header_bytes);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    Some(value.trim().parse::<usize>().unwrap())
                } else {
                    None
                }
            })
            .unwrap_or(0);

        let mut body_bytes = buffer[header_end + 4..].to_vec();
        while body_bytes.len() < content_length {
            let read = stream.read(&mut chunk).unwrap();
            if read == 0 {
                break;
            }
            body_bytes.extend_from_slice(&chunk[..read]);
        }

        tx.send(String::from_utf8(body_bytes).unwrap()).unwrap();

        let response = format!(
            "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
            response_body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
    });

    (address, rx)
}

#[test]
fn ollama_plugin_posts_message_body_to_local_service() {
    let (base_url, request_rx) = spawn_fake_ollama(
        "200 OK",
        r#"{ "response": "Local model says hello", "done": true }"#,
    );
    let binary = env!("CARGO_BIN_EXE_ollama-plugin");

    let output = Command::new(binary)
        .env("OLLAMA_PLUGIN_BASE_URL", &base_url)
        .env("OLLAMA_PLUGIN_MODEL", "gemma3:1b")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"Summarize this patch")?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "Local model says hello"
    );

    let request = request_rx.recv().unwrap();
    assert!(request.contains(r#""model":"gemma3:1b""#));
    assert!(request.contains(r#""prompt":"Summarize this patch""#));
    assert!(request.contains(r#""stream":false"#));
}

#[test]
fn ollama_plugin_reports_json_error_message() {
    let (base_url, _request_rx) = spawn_fake_ollama(
        "500 Internal Server Error",
        r#"{ "error": "model not found" }"#,
    );
    let binary = env!("CARGO_BIN_EXE_ollama-plugin");

    let output = Command::new(binary)
        .env("OLLAMA_PLUGIN_BASE_URL", &base_url)
        .env("OLLAMA_PLUGIN_MODEL", "missing-model")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.take().unwrap().write_all(b"Hello")?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("model not found"));
}

#[test]
fn ollama_plugin_reports_plain_http_error_body() {
    let (base_url, _request_rx) =
        spawn_fake_ollama("502 Bad Gateway", "upstream local model backend failed");
    let binary = env!("CARGO_BIN_EXE_ollama-plugin");

    let output = Command::new(binary)
        .env("OLLAMA_PLUGIN_BASE_URL", &base_url)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.take().unwrap().write_all(b"Hello")?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("upstream local model backend failed")
    );
}
