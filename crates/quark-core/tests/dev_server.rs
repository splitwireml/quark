use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};

#[test]
fn dev_server_announces_port_and_serves() {
    let root = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_quark-dev-server"))
        .args(["--port", "0", "--data-dir"])
        .arg(root.path().join("data"))
        .arg("--cache-dir")
        .arg(root.path().join("cache"))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let address = line
        .trim()
        .strip_prefix("QUARK_LISTENING ")
        .unwrap_or_else(|| panic!("unexpected first line: {line:?}"));

    let mut stream = TcpStream::connect(address).unwrap();
    write!(
        stream,
        "GET /api/projects HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    child.kill().unwrap();
    child.wait().unwrap();

    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
}

#[test]
fn dev_server_rejects_unknown_arguments_with_usage() {
    let output = Command::new(env!("CARGO_BIN_EXE_quark-dev-server"))
        .arg("--bogus")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage"));
}
