use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::bencode::{self, Value, dict};

static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);

pub fn next_request_id() -> String {
    let id = NEXT_REQUEST_ID.fetch_add(1, Ordering::SeqCst);
    format!("perry-{id}")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TestKind {
    Namespace { ns: String },
    TestAtCursor { var: String },
    All,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RequestKind {
    CloneSession,
    LoadFile {
        path: String,
    },
    SwitchNs {
        ns: String,
    },
    TestVarQuery {
        kind: TestKind,
        path: Option<String>,
    },
    Info {
        symbol: String,
    },
    Generic,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRequest {
    pub id: String,
    pub kind: RequestKind,
    pub responses: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedResponse {
    pub id: String,
    pub kind: RequestKind,
    pub messages: Vec<Value>,
}

impl CompletedResponse {
    pub fn value(&self) -> Option<String> {
        let mut last_val = None;
        for msg in &self.messages {
            if let Some(val) = msg.get("value").and_then(Value::as_str) {
                last_val = Some(val.to_string());
            }
        }
        last_val
    }

    pub fn out(&self) -> String {
        let mut out = String::new();
        for msg in &self.messages {
            if let Some(s) = msg.get("out").and_then(Value::as_str) {
                out.push_str(s);
            }
        }
        out
    }

    pub fn err(&self) -> String {
        let mut err = String::new();
        for msg in &self.messages {
            if let Some(s) = msg.get("err").and_then(Value::as_str) {
                err.push_str(s);
            }
        }
        err
    }

    pub fn exception(&self) -> Option<String> {
        for msg in &self.messages {
            if let Some(ex) = msg
                .get("ex")
                .and_then(Value::as_str)
                .or_else(|| msg.get("root-ex").and_then(Value::as_str))
            {
                return Some(ex.to_string());
            }
        }
        None
    }

    pub fn ns(&self) -> Option<String> {
        for msg in &self.messages {
            if let Some(ns) = msg.get("ns").and_then(Value::as_str) {
                return Some(ns.to_string());
            }
        }
        None
    }

    pub fn new_session(&self) -> Option<String> {
        for msg in &self.messages {
            if let Some(s) = msg.get("new-session").and_then(Value::as_str) {
                return Some(s.to_string());
            }
        }
        None
    }
}

pub struct NreplClientState {
    pub session_id: Option<String>,
    pub current_ns: String,
    pub host: String,
    pub port: u16,
    pub stream_id: Option<u64>,
    pub rx_buffer: Vec<u8>,
    pub pending_requests: HashMap<String, PendingRequest>,
}

impl NreplClientState {
    pub fn new(host: String, port: u16) -> Self {
        Self {
            session_id: None,
            current_ns: "user".to_string(),
            host,
            port,
            stream_id: None,
            rx_buffer: Vec::new(),
            pending_requests: HashMap::new(),
        }
    }

    pub fn register_request(&mut self, id: String, kind: RequestKind) {
        self.pending_requests.insert(
            id.clone(),
            PendingRequest {
                id,
                kind,
                responses: Vec::new(),
            },
        );
    }

    /// Appends incoming TCP bytes, parses bencode frames, and returns any completed responses
    /// along with any unprompted out/err messages.
    pub fn feed_bytes(
        &mut self,
        bytes: &[u8],
    ) -> Result<(Vec<CompletedResponse>, Vec<(String, bool)>), String> {
        self.rx_buffer.extend_from_slice(bytes);
        let values = bencode::decode_available(&mut self.rx_buffer)?;

        let mut completed = Vec::new();
        let mut unprompted_outputs = Vec::new();

        for msg in values {
            let id = msg
                .get("id")
                .and_then(Value::as_str)
                .map(|s| s.to_string());

            let is_done = msg
                .get("status")
                .and_then(Value::as_list)
                .map(|list| list.iter().any(|item| item.as_str() == Some("done")))
                .unwrap_or(false);

            if let Some(ref req_id) = id {
                if let Some(pending) = self.pending_requests.get_mut(req_id) {
                    pending.responses.push(msg.clone());
                    if is_done {
                        if let Some(req) = self.pending_requests.remove(req_id) {
                            completed.push(CompletedResponse {
                                id: req.id,
                                kind: req.kind,
                                messages: req.responses,
                            });
                        }
                    }
                    continue;
                }
            }

            if let Some(out) = msg.get("out").and_then(Value::as_str) {
                unprompted_outputs.push((out.to_string(), false));
            }
            if let Some(err) = msg.get("err").and_then(Value::as_str) {
                unprompted_outputs.push((err.to_string(), true));
            }
        }

        Ok((completed, unprompted_outputs))
    }
}

pub fn clone_session_request(id: &str) -> Value {
    dict([("op", Value::str("clone")), ("id", Value::str(id))])
}

pub fn eval_request(
    id: &str,
    session: Option<&str>,
    ns: Option<&str>,
    code: &str,
) -> Value {
    let mut entries = vec![
        ("op", Value::str("eval")),
        ("id", Value::str(id)),
        ("code", Value::str(code)),
    ];
    if let Some(s) = session {
        entries.push(("session", Value::str(s)));
    }
    if let Some(n) = ns {
        entries.push(("ns", Value::str(n)));
    }
    dict(entries)
}

pub fn load_file_request(
    id: &str,
    session: Option<&str>,
    file_content: &str,
    file_path: &str,
    file_name: &str,
) -> Value {
    let mut entries = vec![
        ("op", Value::str("load-file")),
        ("id", Value::str(id)),
        ("file", Value::str(file_content)),
        ("file-path", Value::str(file_path)),
        ("file-name", Value::str(file_name)),
    ];
    if let Some(s) = session {
        entries.push(("session", Value::str(s)));
    }
    dict(entries)
}

pub fn info_request(id: &str, session: Option<&str>, ns: &str, symbol: &str) -> Value {
    let mut entries = vec![
        ("op", Value::str("info")),
        ("id", Value::str(id)),
        ("ns", Value::str(ns)),
        ("sym", Value::str(symbol)),
    ];
    if let Some(s) = session {
        entries.push(("session", Value::str(s)));
    }
    dict(entries)
}

pub fn test_var_query_request(id: &str, session: &str, var_query: Value) -> Value {
    dict([
        ("op", Value::str("test-var-query")),
        ("id", Value::str(id)),
        ("session", Value::str(session)),
        ("var-query", var_query),
    ])
}
