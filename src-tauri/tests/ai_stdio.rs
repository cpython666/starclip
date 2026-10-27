#![cfg(feature = "desktop")]
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    process::{Command, Stdio},
    thread,
};

// Actual distributed executable, inherited pipes, original fixed data and an
// ephemeral loopback broker. This never launches Tauri or accesses user storage.
#[test]
fn distributed_binary_runs_mcp_and_cli_without_opening_the_desktop() {
    // CI repeats this test against the release binary after packaging.
    let binary = std::env::var_os("STARCLIP_TEST_BINARY")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_starclip").into());
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let token = "c".repeat(64);
    let server_token = token.clone();
    let server = thread::spawn(move || {
        for expected in ["status", "pet_signal", "status", "pet_clear"] {
            let (mut stream, _) = listener.accept().unwrap();
            let mut line = String::new();
            BufReader::new(&mut stream).read_line(&mut line).unwrap();
            let req: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(req["token"], server_token);
            assert_eq!(req["action"], expected);
            let response = match expected {
                "pet_signal" => {
                    assert_eq!(
                        req["arguments"],
                        json!({"status":"working","message":"原创星伴消息","durationSeconds":5})
                    );
                    json!({"accepted":true,"revision":1})
                }
                "pet_clear" => {
                    assert_eq!(req["arguments"], json!({}));
                    json!({"cleared":true,"revision":2})
                }
                _ => {
                    json!({"available":true,"clipboardReadOnly":true,"allowPetControl":true,"contentTrust":"untrusted-user-data"})
                }
            };
            writeln!(stream, "{}", json!({"result":response})).unwrap();
        }
    });
    let mut child = Command::new(&binary)
        .args(["mcp", "--port", &port.to_string()])
        .env("STARCLIP_AI_TOKEN", &token)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    writeln!(input,"{}",json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"fixture","version":"1"}}})).unwrap();
    input.flush().unwrap();
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let response: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    writeln!(input,"{}",json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"status","arguments":{}}})).unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    let response: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(response["result"]["isError"], false);
    assert_eq!(
        response["result"]["structuredContent"]["clipboardReadOnly"],
        true
    );
    writeln!(input,"{}",json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"pet_signal","arguments":{"status":"working","message":"原创星伴消息","durationSeconds":5}}})).unwrap();
    input.flush().unwrap();
    line.clear();
    output.read_line(&mut line).unwrap();
    let response: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(response["result"]["isError"], false);
    assert_eq!(response["result"]["structuredContent"]["accepted"], true);
    drop(input);
    assert!(child.wait().unwrap().success());
    let cli = Command::new(&binary)
        .args(["ai", "--port", &port.to_string(), "status"])
        .env("STARCLIP_AI_TOKEN", &token)
        .output()
        .unwrap();
    assert!(cli.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&cli.stdout).unwrap()["clipboardReadOnly"],
        true
    );
    assert!(cli.stderr.is_empty());
    let clear = Command::new(&binary)
        .args(["ai", "--port", &port.to_string(), "pet_clear"])
        .env("STARCLIP_AI_TOKEN", &token)
        .output()
        .unwrap();
    assert!(clear.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&clear.stdout).unwrap()["cleared"],
        true
    );
    assert!(clear.stderr.is_empty());
    server.join().unwrap();
}
