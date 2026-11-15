//! Real loopback HTTP and TLS acceptance. All identities, keys and bodies are synthetic.
use super::*;
use crate::{
    protocol::MAX_BODY,
    transport::{Transport, client_builder},
};
use reqwest::Url;
use std::{
    collections::BTreeMap,
    io::{BufRead, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

const DEVICE: &str = "f9168c5e-ceb2-4faa-b6bf-329bf39fa1e4";
const TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const GITHUB_TOKEN: &str = "gho_synthetic_access_token_1234567890";
const START: &str = r#"{"device_code":"0123456789abcdef0123456789abcdef01234567","user_code":"WDJB-MJHT","verification_uri":"https://github.com/login/device","expires_in":900,"interval":5}"#;
const JOINED: &str = r#"{"token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","login":"synthetic-user"}"#;

struct Request {
    method: String,
    target: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}
enum Reply {
    Json(u16, String),
    Raw(Vec<u8>),
    Stall(Duration),
    Disconnect,
}
impl Reply {
    fn ok(text: &str) -> Self {
        Self::Json(200, text.into())
    }
}
struct Server {
    origin: String,
    requests: mpsc::Receiver<Request>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    address: SocketAddr,
}
impl Server {
    fn start(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (send, requests) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            for reply in replies {
                let (mut stream, _) = listener.accept().unwrap();
                if stopped.load(Ordering::SeqCst) {
                    return;
                }
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let request = read_request(&mut stream);
                send.send(request).unwrap();
                match reply {
                    Reply::Json(status, body) => {
                        let bytes = format!(
                            "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = stream.write_all(bytes.as_bytes());
                    }
                    Reply::Raw(bytes) => {
                        let _ = stream.write_all(&bytes);
                    }
                    Reply::Stall(duration) => {
                        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 128\r\n\r\n");
                        thread::sleep(duration);
                    }
                    Reply::Disconnect => {}
                }
            }
        });
        Self {
            origin: format!("http://{address}/"),
            requests,
            stop,
            worker: Some(worker),
            address,
        }
    }
    fn client(&self) -> BoardClient {
        let origin = Url::parse(&self.origin).unwrap();
        BoardClient {
            transport: Some(Transport {
                client: client_builder().https_only(false).build().unwrap(),
                client_id: "synthetic_client".into(),
                request_timeout: Duration::from_secs(15),
                github_device: origin.join("device/code").unwrap(),
                github_poll: origin.join("oauth/access_token").unwrap(),
                origin,
            }),
        }
    }
    fn request(&self) -> Request {
        self.requests.recv_timeout(Duration::from_secs(2)).unwrap()
    }
    fn no_request(&self) {
        assert!(matches!(
            self.requests.try_recv(),
            Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected)
        ));
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_millis(100));
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
fn read_request(stream: &mut TcpStream) -> Request {
    let mut bytes = Vec::new();
    let end = loop {
        let mut buffer = [0; 2048];
        let n = stream.read(&mut buffer).unwrap();
        assert_ne!(n, 0);
        bytes.extend_from_slice(&buffer[..n]);
        assert!(bytes.len() <= MAX_BODY + 8192);
        if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break index + 4;
        }
    };
    let header = std::str::from_utf8(&bytes[..end]).unwrap();
    let mut lines = header.split("\r\n");
    let mut first = lines.next().unwrap().split_whitespace();
    let method = first.next().unwrap().to_owned();
    let target = first.next().unwrap().to_owned();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_owned()))
        .collect::<BTreeMap<_, _>>();
    let length = headers
        .get("content-length")
        .map_or(0, |n| n.parse::<usize>().unwrap());
    assert!(length <= MAX_BODY);
    while bytes.len() - end < length {
        let mut buffer = [0; 2048];
        let n = stream.read(&mut buffer).unwrap();
        assert_ne!(n, 0);
        bytes.extend_from_slice(&buffer[..n]);
    }
    Request {
        method,
        target,
        headers,
        body: bytes[end..end + length].to_vec(),
    }
}
fn device() -> DeviceId {
    DeviceId::parse(DEVICE).unwrap()
}
fn service_token() -> ServiceToken {
    ServiceToken::from_secret(Zeroizing::new(TOKEN.into())).unwrap()
}
fn github_token() -> GithubToken {
    GithubToken::from_secret(Zeroizing::new(GITHUB_TOKEN.into())).unwrap()
}
fn approved() -> ApprovedSyncBody {
    let bytes = format!("{{ \"deviceId\":\"{DEVICE}\", \"rows\":[{{\"date\":\"2026-11-13\",\"tool\":\"codex\",\"model\":\"gpt-5\",\"inputTokens\":20,\"outputTokens\":3,\"cacheReadTokens\":2,\"cacheWriteTokens\":0,\"totalTokens\":25,\"estimatedValueMicros\":null}}] }}").into_bytes();
    ApprovedSyncBody::from_json(bytes, "2026-11-13T10:00:00Z".parse().unwrap()).unwrap()
}

#[test]
fn service_requests_keep_exact_approved_bytes_and_typed_exact_rank_values() {
    let rank = r#"{"title":"Self-reported","period":"all","metric":"value","snapshotAt":"2026-11-13T10:00:00.000Z","totalParticipants":144,"entry":{"rank":117,"login":"synthetic-user","totalTokens":9007199254740991,"estimatedValueMicros":null}}"#;
    let server = Server::start(vec![
        Reply::ok(JOINED),
        Reply::ok(r#"{"accepted":1}"#),
        Reply::ok(rank),
        Reply::ok(r#"{"left":true}"#),
    ]);
    let client = server.client();
    let joined = client.join(github_token(), &device()).unwrap();
    assert_eq!(joined.login, "synthetic-user");
    let join = server.request();
    assert_eq!(
        (join.method.as_str(), join.target.as_str()),
        ("POST", "/v1/join")
    );
    assert!(!join.headers.contains_key("authorization"));
    let body: serde_json::Value = serde_json::from_slice(&join.body).unwrap();
    assert_eq!(
        body,
        serde_json::json!({"githubToken":GITHUB_TOKEN,"deviceId":DEVICE})
    );
    let body = approved();
    assert_eq!(client.sync(&joined.token, &body).unwrap(), 1);
    let sync = server.request();
    assert_eq!(sync.target, "/v1/sync");
    assert_eq!(sync.body, body.as_bytes());
    assert_eq!(sync.headers["authorization"], format!("Bearer {TOKEN}"));
    assert_eq!(sync.headers["accept-encoding"], "identity");
    let rank = client
        .own_rank(&joined.token, Period::All, Metric::Value)
        .unwrap();
    assert_eq!(rank.total_participants, 144);
    assert_eq!(rank.entry.as_ref().unwrap().rank, 117);
    assert_eq!(
        rank.entry.as_ref().unwrap().total_tokens,
        9_007_199_254_740_991
    );
    assert_eq!(rank.entry.as_ref().unwrap().estimated_value_micros, None);
    let request = server.request();
    assert_eq!(request.target, "/v1/me/rank?period=all&metric=value");
    client.leave(&joined.token).unwrap();
    assert_eq!(server.request().target, "/v1/leave");
    for debug in [
        format!("{client:?}"),
        format!("{joined:?}"),
        format!("{rank:?}"),
        format!("{body:?}"),
    ] {
        for private in [TOKEN, GITHUB_TOKEN, DEVICE, "synthetic-user", "gpt-5"] {
            assert!(!debug.contains(private));
        }
    }
}

#[test]
fn device_flow_enforces_monotonic_pending_slowdown_terminal_and_scope_rules() {
    let token = format!(r#"{{"access_token":"{GITHUB_TOKEN}","token_type":"bearer","scope":""}}"#);
    let server = Server::start(vec![
        Reply::ok(START),
        Reply::ok(r#"{"error":"authorization_pending"}"#),
        Reply::ok(r#"{"error":"slow_down","interval":12}"#),
        Reply::ok(&token),
    ]);
    let client = server.client();
    let mut flow = client.device_start(Duration::from_secs(10)).unwrap();
    let start = server.request();
    assert_eq!(start.body, b"client_id=synthetic_client");
    assert_eq!(start.target, "/device/code");
    assert_eq!(flow.user_code(), "WDJB-MJHT");
    assert_eq!(flow.verification_uri(), "https://github.com/login/device");
    assert_eq!(
        client
            .device_poll(&mut flow, Duration::from_secs(14))
            .unwrap_err(),
        Error::PollTooEarly {
            retry_at: Duration::from_secs(15)
        }
    );
    server.no_request();
    assert!(
        matches!(client.device_poll(&mut flow,Duration::from_secs(15)).unwrap(),PollOutcome::Pending { retry_at } if retry_at==Duration::from_secs(20))
    );
    let poll = server.request();
    assert!(
        String::from_utf8(poll.body)
            .unwrap()
            .contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code")
    );
    assert!(
        matches!(client.device_poll(&mut flow,Duration::from_secs(20)).unwrap(),PollOutcome::SlowDown { retry_at } if retry_at==Duration::from_secs(32))
    );
    server.request();
    assert_eq!(
        client
            .device_poll(&mut flow, Duration::from_secs(19))
            .unwrap_err(),
        Error::ClockWentBackwards
    );
    assert!(matches!(
        client.device_poll(&mut flow, Duration::from_secs(31)),
        Err(Error::PollTooEarly { .. })
    ));
    server.no_request();
    let token = client
        .device_poll(&mut flow, Duration::from_secs(32))
        .unwrap();
    assert!(matches!(token, PollOutcome::Authorized(_)));
    server.request();
    assert_eq!(
        client
            .device_poll(&mut flow, Duration::from_secs(40))
            .unwrap_err(),
        Error::FlowFinished
    );
    assert!(!format!("{token:?} {flow:?}").contains(GITHUB_TOKEN));
    server.no_request();

    for (status, response) in [
        (400, r#"{"error":"access_denied"}"#.to_owned()),
        (
            200,
            format!(
                r#"{{"access_token":"{GITHUB_TOKEN}","token_type":"bearer","scope":"repo,user:email"}}"#
            ),
        ),
        (
            400,
            format!(r#"{{"access_token":"{GITHUB_TOKEN}","token_type":"bearer","scope":""}}"#),
        ),
    ] {
        let server = Server::start(vec![Reply::ok(START), Reply::Json(status, response)]);
        let client = server.client();
        let mut flow = client.device_start(Duration::ZERO).unwrap();
        server.request();
        let result = client.device_poll(&mut flow, Duration::from_secs(5));
        server.request();
        assert!(matches!(
            result,
            Ok(PollOutcome::Denied) | Err(Error::UnexpectedResponse)
        ));
        assert_eq!(
            client
                .device_poll(&mut flow, Duration::from_secs(10))
                .unwrap_err(),
            Error::FlowFinished
        );
        server.no_request();
    }
    let server = Server::start(vec![Reply::ok(START)]);
    let client = server.client();
    let mut flow = client.device_start(Duration::ZERO).unwrap();
    server.request();
    assert!(matches!(
        client
            .device_poll(&mut flow, Duration::from_secs(900))
            .unwrap(),
        PollOutcome::Expired
    ));
    server.no_request();
}

#[test]
fn invalid_sync_bodies_never_cross_the_privacy_boundary() {
    let server = Server::start(vec![Reply::ok(r#"{"accepted":1}"#)]);
    let client = server.client();
    let good = String::from_utf8(approved().as_bytes().to_vec()).unwrap();
    let now = "2026-11-13T10:00:00Z".parse().unwrap();
    let mut invalid = vec![
        good.replacen("{ ", "{ \"email\":\"private@example.invalid\",", 1),
        good.replace(
            "\"model\":\"gpt-5\"",
            "\"model\":\"private@example.invalid\"",
        ),
        good.replace("\"model\":\"gpt-5\"", "\"model\":\"/srv/private-project\""),
        good.replace("\"date\":\"2026-11-13\"", "\"date\":\"2026-11-14\""),
        good.replace("\"date\":\"2026-11-13\"", "\"date\":\"2020-01-01\""),
        good.replace("\"inputTokens\":20", "\"inputTokens\":\"20\""),
        good.replace("\"inputTokens\":20", "\"inputTokens\":1000000000001"),
        good.replace(
            "\"estimatedValueMicros\":null",
            "\"providerAccountId\":\"private-id\"",
        ),
        good.replace(
            "\"inputTokens\":20",
            "\"inputTokens\":20,\"inputTokens\":20",
        ),
        good.replacen(
            "\"deviceId\":",
            &format!("\"deviceId\":\"{DEVICE}\",\"deviceId\":"),
            1,
        ),
    ];
    let body: serde_json::Value = serde_json::from_str(&good).unwrap();
    let mut doubled = body.clone();
    doubled["rows"]
        .as_array_mut()
        .unwrap()
        .push(body["rows"][0].clone());
    invalid.push(doubled.to_string());
    let mut too_many = body;
    too_many["rows"] = serde_json::Value::Array(
        (0..401)
            .map(|n| {
                let mut value = too_many["rows"][0].clone();
                value["model"] = format!("model-{n}").into();
                value
            })
            .collect(),
    );
    invalid.push(too_many.to_string());
    invalid.push(" ".repeat(MAX_BODY + 1));
    for bytes in invalid {
        assert!(ApprovedSyncBody::from_json(bytes.into_bytes(), now).is_err());
        server.no_request();
    }
    let body = approved();
    assert_eq!(client.sync(&service_token(), &body).unwrap(), 1);
    assert_eq!(server.request().body, body.as_bytes());
}

#[test]
fn redirects_errors_compression_and_response_byte_caps_use_safe_failures() {
    let target = Server::start(vec![Reply::ok(JOINED)]);
    let redirect = format!(
        "HTTP/1.1 307 Redirect\r\nLocation: {}steal\r\nContent-Length: 0\r\n\r\n",
        target.origin
    );
    let server = Server::start(vec![Reply::Raw(redirect.into_bytes())]);
    assert_eq!(
        server.client().join(github_token(), &device()).unwrap_err(),
        Error::RedirectRefused
    );
    server.request();
    target.no_request();
    let cases=vec![
        (Reply::Json(401,r#"{"error":"not_joined"}"#.into()),Error::NotJoined),
        (Reply::Json(429,r#"{"error":"rate_limited"}"#.into()),Error::RateLimited),
        (Reply::Json(500,r#"{"error":"unavailable"}"#.into()),Error::ServiceUnavailable),
        (Reply::Json(200,format!(r#"{{"token":"{TOKEN}","login":"synthetic-user","prompt":"private-secret-content"}}"#)),Error::UnexpectedResponse),
        (Reply::Raw(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",MAX_BODY+1).into_bytes()),Error::ResponseTooLarge),
        (Reply::Raw(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",MAX_BODY+1,"x".repeat(MAX_BODY+1)).into_bytes()),Error::ResponseTooLarge),
        (Reply::Raw(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\n\r\n{JOINED}",JOINED.len()).into_bytes()),Error::UnexpectedResponse),
        (Reply::Raw(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1\r\n\r\n\xff".to_vec()),Error::UnexpectedResponse),
    ];
    for (reply, expected) in cases {
        let server = Server::start(vec![reply]);
        let error = server.client().join(github_token(), &device()).unwrap_err();
        assert_eq!(error, expected);
        server.request();
        server.no_request();
        for private in [TOKEN, GITHUB_TOKEN, "private-secret-content"] {
            assert!(!format!("{error:?} {error}").contains(private));
        }
    }
    let server = Server::start(vec![Reply::Disconnect, Reply::ok(JOINED)]);
    assert_eq!(
        server.client().join(github_token(), &device()).unwrap_err(),
        Error::Transport
    );
    server.request();
    server.no_request();
}

#[test]
fn the_request_deadline_covers_stalled_response_reads() {
    let server = Server::start(vec![Reply::Stall(Duration::from_millis(400))]);
    let mut client = server.client();
    client.transport.as_mut().unwrap().request_timeout = Duration::from_millis(100);
    client.transport.as_mut().unwrap().client = client_builder()
        .https_only(false)
        .timeout(Duration::from_millis(100))
        .build()
        .unwrap();
    let start = Instant::now();
    assert_eq!(
        client.join(github_token(), &device()).unwrap_err(),
        Error::Timeout
    );
    assert!(start.elapsed() < Duration::from_secs(2));
    server.request();
}

#[test]
fn unconfigured_build_paths_and_invalid_origins_never_send_requests() {
    let server = Server::start(vec![Reply::ok(START)]);
    let client = BoardClient { transport: None };
    assert!(!client.is_configured());
    assert!(client.service_origin().is_none());
    assert_eq!(
        client.device_start(Duration::ZERO).unwrap_err(),
        Error::Unconfigured
    );
    assert_eq!(
        client.join(github_token(), &device()).unwrap_err(),
        Error::Unconfigured
    );
    assert_eq!(
        client.sync(&service_token(), &approved()).unwrap_err(),
        Error::Unconfigured
    );
    assert_eq!(
        client.leave(&service_token()).unwrap_err(),
        Error::Unconfigured
    );
    assert_eq!(
        client
            .own_rank(&service_token(), Period::All, Metric::Tokens)
            .unwrap_err(),
        Error::Unconfigured
    );
    server.no_request();
    for origin in [
        "http://example.invalid",
        "https://user:password@example.invalid",
        "https://@example.invalid",
        "https://example.invalid/path",
        "https://example.invalid/?token=private",
        "https://example.invalid/#fragment",
        "https:example.invalid",
        "https://example.invalid\\other",
    ] {
        assert!(matches!(
            Transport::production(origin, "synthetic_client"),
            Err(Error::InvalidConfiguration)
        ));
    }
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.0.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
            thread::yield_now();
        }
    }
}
const TLS_SERVER: &str = r#"
import http.server, ssl, sys
class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args): pass
    def do_POST(self):
        self.rfile.read(int(self.headers.get('Content-Length', '0')))
        body=b'{"token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","login":"synthetic-user"}'
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)
server=http.server.HTTPServer(('127.0.0.1',0),Handler)
context=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
context.load_cert_chain(sys.argv[1],sys.argv[2])
server.socket=context.wrap_socket(server.socket,server_side=True)
print(server.server_port,flush=True)
server.serve_forever()
"#;

#[test]
fn production_tls_checks_reject_unknown_ca_and_accept_an_explicit_test_trust_root() {
    let directory = tempfile::tempdir().unwrap();
    let cert = directory.path().join("synthetic-cert.pem");
    let key = directory.path().join("synthetic-key.pem");
    let child = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-subj",
            "/CN=localhost",
            "-addext",
            "subjectAltName=DNS:localhost,IP:127.0.0.1",
            "-days",
            "1",
            "-keyout",
        ])
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("TLS acceptance requires the openssl executable");
    let mut generation = ChildGuard(child);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = generation.0.try_wait().unwrap() {
            assert!(
                status.success(),
                "synthetic TLS certificate creation failed"
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "synthetic TLS certificate creation exceeded five seconds"
        );
        thread::yield_now();
    }
    let child = Command::new("python3")
        .args(["-u", "-c", TLS_SERVER])
        .arg(&cert)
        .arg(&key)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("TLS acceptance requires Python 3 with ssl");
    let mut server = ChildGuard(child);
    let stdout = server.0.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    let readiness = thread::spawn(move || {
        let mut text = String::new();
        let result = std::io::BufReader::new(stdout).read_line(&mut text);
        let _ = send.send(result.map(|_| text));
    });
    let port = receive
        .recv_timeout(Duration::from_secs(5))
        .expect("synthetic TLS server did not report readiness")
        .expect("synthetic TLS server readiness failed")
        .trim()
        .parse::<u16>()
        .expect("synthetic TLS server returned an invalid port");
    readiness.join().unwrap();
    let origin = format!("https://127.0.0.1:{port}/");
    let client = BoardClient {
        transport: Some(Transport::production(&origin, "synthetic_client").unwrap()),
    };
    assert_eq!(
        client.join(github_token(), &device()).unwrap_err(),
        Error::Transport
    );
    let mut transport = Transport::production(&origin, "synthetic_client").unwrap();
    // Trust injection exists only in this test. Certificate and hostname checks stay enabled.
    let certificate = reqwest::Certificate::from_pem(&std::fs::read(&cert).unwrap()).unwrap();
    transport.client = client_builder()
        .add_root_certificate(certificate)
        .build()
        .unwrap();
    let client = BoardClient {
        transport: Some(transport),
    };
    assert_eq!(
        client.join(github_token(), &device()).unwrap().login,
        "synthetic-user"
    );
}

#[test]
fn rank_wire_replies_reject_unsafe_counts_missing_nulls_and_untrusted_metadata() {
    let good = serde_json::json!({"title":"Self-reported","period":"all","metric":"value","snapshotAt":"2026-11-13T10:00:00.000Z","totalParticipants":144,"entry":{"rank":117,"login":"synthetic-user","totalTokens":7,"estimatedValueMicros":null}});
    let mut cases = Vec::new();
    for (field, value) in [
        ("totalTokens", serde_json::json!(9_007_199_254_740_992u64)),
        ("rank", serde_json::json!(145)),
        ("login", serde_json::json!("private@example.invalid")),
        ("estimatedValueMicros", serde_json::json!(1.5)),
    ] {
        let mut reply = good.clone();
        reply["entry"][field] = value;
        cases.push(reply.to_string());
    }
    let mut reply = good.clone();
    reply["entry"]
        .as_object_mut()
        .unwrap()
        .remove("estimatedValueMicros");
    cases.push(reply.to_string());
    let mut reply = good.clone();
    reply.as_object_mut().unwrap().remove("entry");
    cases.push(reply.to_string());
    let mut reply = good.clone();
    reply["period"] = "7d".into();
    cases.push(reply.to_string());
    let mut reply = good.clone();
    reply["snapshotAt"] = "private-source-content".into();
    cases.push(reply.to_string());
    let mut reply = good;
    reply["email"] = "private@example.invalid".into();
    cases.push(reply.to_string());
    for reply in cases {
        let server = Server::start(vec![Reply::ok(&reply)]);
        let error = server
            .client()
            .own_rank(&service_token(), Period::All, Metric::Value)
            .unwrap_err();
        assert_eq!(error, Error::UnexpectedResponse);
        server.request();
        assert!(!format!("{error:?} {error}").contains("private"));
    }
    let server = Server::start(vec![Reply::ok(
        r#"{"title":"Self-reported","period":"all","metric":"value","snapshotAt":"2026-11-13T10:00:00.000Z","totalParticipants":0,"entry":null}"#,
    )]);
    let rank = server
        .client()
        .own_rank(&service_token(), Period::All, Metric::Value)
        .unwrap();
    assert_eq!(rank.total_participants, 0);
    assert!(rank.entry.is_none());
    server.request();
}

#[test]
fn github_reply_bounds_and_terminal_expiry_do_not_open_untrusted_urls_or_restart() {
    let baseline: serde_json::Value = serde_json::from_str(START).unwrap();
    let cases = [
        (
            "verification_uri",
            serde_json::json!("https://attacker.invalid/login/device"),
        ),
        ("expires_in", serde_json::json!(901)),
        ("expires_in", serde_json::json!(0)),
        ("interval", serde_json::json!(0)),
        ("interval", serde_json::json!(901)),
        ("device_code", serde_json::json!("private-content")),
    ];
    for (field, value) in cases {
        let mut reply = baseline.clone();
        reply[field] = value;
        let server = Server::start(vec![Reply::ok(&reply.to_string())]);
        let error = server.client().device_start(Duration::ZERO).unwrap_err();
        assert_eq!(error, Error::UnexpectedResponse);
        server.request();
        server.no_request();
        assert!(!format!("{error:?} {error}").contains("attacker"));
    }
    for error in ["expired_token", "token_expired"] {
        let reply = format!(
            r#"{{"error":"{error}","error_description":"private-secret-content","error_uri":"https://attacker.invalid/"}}"#
        );
        let server = Server::start(vec![Reply::ok(START), Reply::Json(400, reply)]);
        let client = server.client();
        let mut flow = client.device_start(Duration::ZERO).unwrap();
        server.request();
        assert!(matches!(
            client
                .device_poll(&mut flow, Duration::from_secs(5))
                .unwrap(),
            PollOutcome::Expired
        ));
        server.request();
        assert_eq!(
            client
                .device_poll(&mut flow, Duration::from_secs(10))
                .unwrap_err(),
            Error::FlowFinished
        );
        server.no_request();
        let debug = format!("{flow:?}");
        for private in [
            "WDJB-MJHT",
            "0123456789abcdef0123456789abcdef01234567",
            "private-secret-content",
            "attacker",
        ] {
            assert!(!debug.contains(private));
        }
    }
    let server = Server::start(vec![
        Reply::ok(START),
        Reply::ok(r#"{"error":"slow_down","interval":1}"#),
        Reply::ok(r#"{"error":"access_denied"}"#),
    ]);
    let client = server.client();
    let mut flow = client.device_start(Duration::ZERO).unwrap();
    server.request();
    assert!(
        matches!(client.device_poll(&mut flow,Duration::from_secs(5)).unwrap(),PollOutcome::SlowDown {retry_at} if retry_at==Duration::from_secs(15))
    );
    server.request();
    assert!(matches!(
        client.device_poll(&mut flow, Duration::from_secs(14)),
        Err(Error::PollTooEarly { .. })
    ));
    server.no_request();
    assert!(matches!(
        client
            .device_poll(&mut flow, Duration::from_secs(15))
            .unwrap(),
        PollOutcome::Denied
    ));
    server.request();
}

#[test]
fn failed_poll_requests_keep_the_interval_without_an_automatic_retry() {
    let server = Server::start(vec![
        Reply::ok(START),
        Reply::Disconnect,
        Reply::ok(r#"{"error":"access_denied"}"#),
    ]);
    let client = server.client();
    let mut flow = client.device_start(Duration::ZERO).unwrap();
    server.request();
    assert_eq!(
        client
            .device_poll(&mut flow, Duration::from_secs(5))
            .unwrap_err(),
        Error::Transport
    );
    server.request();
    assert_eq!(
        client
            .device_poll(&mut flow, Duration::from_secs(6))
            .unwrap_err(),
        Error::PollTooEarly {
            retry_at: Duration::from_secs(10)
        }
    );
    server.no_request();
    assert!(matches!(
        client
            .device_poll(&mut flow, Duration::from_secs(10))
            .unwrap(),
        PollOutcome::Denied
    ));
    server.request();
}

#[test]
fn expiring_github_reply_is_consumed_once_and_partial_expiry_refuses() {
    for (extra, valid) in [
        (
            r#","refresh_token":"ghr_synthetic_refresh_token_1234567890","expires_in":28800,"refresh_token_expires_in":15897600"#,
            true,
        ),
        (
            r#","refresh_token":"ghr_synthetic_refresh_token_1234567890""#,
            false,
        ),
        (
            r#","refresh_token":null,"expires_in":28800,"refresh_token_expires_in":15897600"#,
            false,
        ),
        (
            r#","refresh_token":"ghr_synthetic_refresh_token_1234567890","expires_in":0,"refresh_token_expires_in":15897600"#,
            false,
        ),
    ] {
        let response = format!(
            r#"{{"access_token":"{GITHUB_TOKEN}","token_type":"bearer","scope":""{extra}}}"#
        );
        let server = Server::start(vec![Reply::ok(START), Reply::ok(&response)]);
        let client = server.client();
        let mut flow = client.device_start(Duration::ZERO).unwrap();
        server.request();
        let result = client.device_poll(&mut flow, Duration::from_secs(5));
        server.request();
        assert_eq!(matches!(result, Ok(PollOutcome::Authorized(_))), valid);
        assert!(!format!("{result:?}").contains("ghr_synthetic"));
        assert_eq!(
            client
                .device_poll(&mut flow, Duration::from_secs(10))
                .unwrap_err(),
            Error::FlowFinished
        );
        server.no_request();
    }
}
