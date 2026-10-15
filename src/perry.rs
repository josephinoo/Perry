pub mod bencode;
pub mod commands;
pub mod forms;
pub mod jack_in;
pub mod nrepl;
pub mod paredit;
pub mod tests;

use zed_extension_api::{
    self as zed,
    CommandContext,
    Extension,
    editor::{Decoration, DecorationStyle, GutterDecoration, InlineDecoration, Range as ZedRange},
    ui::NotificationLevel,
};

use crate::bencode::Value;
use crate::nrepl::{CompletedResponse, NreplClientState, RequestKind};

pub struct PerryExtension {
    pub nrepl: Option<NreplClientState>,
    pub tcp_stream: Option<zed::tcp::TcpStream>,
    pub jack_in_handle: Option<u64>,
    pub jack_in_buffer: String,
}

impl PerryExtension {
    pub fn new() -> Self {
        Self {
            nrepl: None,
            tcp_stream: None,
            jack_in_handle: None,
            jack_in_buffer: String::new(),
        }
    }

    pub fn connect_to(&mut self, host: &str, port: u16) -> Result<(), String> {
        let stream = zed::tcp::connect(host, port)?;
        let stream_id = stream.subscribe()?;

        let mut state = NreplClientState::new(host.to_string(), port);
        state.stream_id = Some(stream_id);

        let clone_id = nrepl::next_request_id();
        state.register_request(clone_id.clone(), RequestKind::CloneSession);
        let clone_req = nrepl::clone_session_request(&clone_id);
        stream.write(&bencode::encode(&clone_req))?;

        self.tcp_stream = Some(stream);
        self.nrepl = Some(state);
        Ok(())
    }

    pub fn ensure_connected(&mut self) -> Result<(), String> {
        if self.tcp_stream.is_some() && self.nrepl.is_some() {
            return Ok(());
        }
        let port_content = zed::panel::read_active_worktree_file(".nrepl-port")
            .map_err(|_| "Could not find .nrepl-port file; run jack-in first".to_string())?;
        let port = port_content
            .trim()
            .parse::<u16>()
            .map_err(|_| "Invalid port in .nrepl-port".to_string())?;
        self.connect_to("127.0.0.1", port)
    }

    pub fn handle_completed_response(
        &mut self,
        response: CompletedResponse,
    ) -> Result<(), String> {
        match response.kind {
            RequestKind::CloneSession => {
                if let Some(new_session) = response.new_session() {
                    if let Some(nrepl) = &mut self.nrepl {
                        nrepl.session_id = Some(new_session);
                    }
                }
            }
            RequestKind::Info { symbol } => {
                let mut doc_text = format!("-------------------------\n{symbol}");
                for msg in &response.messages {
                    if let Some(name) = msg.get("name").and_then(Value::as_str) {
                        let ns = msg.get("ns").and_then(Value::as_str).unwrap_or("");
                        doc_text = format!("-------------------------\n{ns}/{name}");
                    }
                    if let Some(arglists) = msg.get("arglists-str").and_then(Value::as_str) {
                        doc_text.push_str(&format!("\n{arglists}"));
                    }
                    if let Some(doc) = msg.get("doc").and_then(Value::as_str) {
                        doc_text.push_str(&format!("\n  {doc}"));
                    }
                }
                zed::ui::notify(
                    NotificationLevel::Info,
                    &doc_text,
                );
            }
            RequestKind::TestVarQuery { path, .. } => {
                match tests::parse_test_response(&response.messages) {
                    Ok(report) => {
                        let mut decorations = Vec::new();
                        for (_ns, vars) in &report.results {
                            for (_var, assertions) in vars {
                                for assertion in assertions {
                                    if assertion.result_type != tests::TestResultType::Pass {
                                        if let Some(line) = assertion.line {
                                            decorations.push(Decoration::InlineAfter(
                                                InlineDecoration {
                                                    range: ZedRange {
                                                        start: line,
                                                        end: line,
                                                    },
                                                    text: format!(
                                                        " => FAIL: {}",
                                                        assertion
                                                            .expected
                                                            .as_deref()
                                                            .unwrap_or("assertion failed")
                                                    ),
                                                    style: DecorationStyle::Error,
                                                },
                                            ));
                                        }
                                    } else if let Some(line) = assertion.line {
                                        decorations.push(Decoration::Gutter(GutterDecoration {
                                            row: line,
                                            style: DecorationStyle::Success,
                                        }));
                                    }
                                }
                            }
                        }

                        if let Some(path) = &path {
                            if !decorations.is_empty() {
                                zed::editor::set_decorations(
                                    path,
                                    "perry-tests",
                                    &decorations,
                                )?;
                            }
                        }

                        let toast = tests::format_toast_summary(&report.summary);
                        let level = if report.summary.fail > 0 || report.summary.error > 0 {
                            NotificationLevel::Warning
                        } else {
                            NotificationLevel::Info
                        };
                        zed::ui::notify(level, &toast);
                    }
                    Err(err) => {
                        zed::ui::notify(NotificationLevel::Error, &err);
                    }
                }
            }
            RequestKind::LoadFile { path } => {
                zed::ui::notify(NotificationLevel::Info, &format!("Loaded {path}"));
            }
            RequestKind::SwitchNs { ns } => {
                if let Some(nrepl) = &mut self.nrepl {
                    nrepl.current_ns = ns.clone();
                }
                zed::ui::notify(NotificationLevel::Info, &format!("Switched namespace to {ns}"));
            }
            RequestKind::Generic => {}
        }
        Ok(())
    }
}

impl Extension for PerryExtension {
    fn new() -> Self {
        PerryExtension::new()
    }

    fn run_command(&mut self, command_id: &str, context: CommandContext) -> Result<(), String> {
        commands::run_command(self, command_id, context)
    }

    fn ui_response(&mut self, _request_id: u64, _selected: Option<String>) -> Result<(), String> {
        Ok(())
    }

    fn process_output(
        &mut self,
        handle: u64,
        _is_stderr: bool,
        text: &str,
    ) -> Result<(), String> {
        if self.jack_in_handle == Some(handle) {
            self.jack_in_buffer.push_str(text);
            if let Some(port) = jack_in::parse_port_from_output(&self.jack_in_buffer) {
                zed::ui::notify(
                    NotificationLevel::Info,
                    &format!("nREPL listo en el puerto {port}: usa repl: refresh kernelspecs"),
                );
            }
        }
        Ok(())
    }

    fn process_exit(&mut self, handle: u64, _code: Option<i32>) -> Result<(), String> {
        if self.jack_in_handle == Some(handle) {
            self.jack_in_handle = None;
        }
        Ok(())
    }

    fn tcp_data(&mut self, stream_id: u64, data: &[u8]) -> Result<(), String> {
        let Some(nrepl) = &mut self.nrepl else {
            return Ok(());
        };
        if nrepl.stream_id != Some(stream_id) {
            return Ok(());
        }

        let (completed, _) = nrepl.feed_bytes(data)?;

        for resp in completed {
            self.handle_completed_response(resp)?;
        }

        Ok(())
    }

    fn tcp_closed(&mut self, stream_id: u64, _error: Option<String>) -> Result<(), String> {
        if let Some(nrepl) = &self.nrepl {
            if nrepl.stream_id == Some(stream_id) {
                self.tcp_stream = None;
                self.nrepl = None;
            }
        }
        Ok(())
    }

    fn on_event(&mut self, kind: &str, _payload_json: &str) -> Result<(), String> {
        match kind {
            "active-editor-changed" | "buffer-saved" => {
                if let Some(editor) = zed::editor::active_editor() {
                    if let Some(ns) = forms::parse_namespace(&editor.text) {
                        if let Some(nrepl) = &mut self.nrepl {
                            nrepl.current_ns = ns;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

zed_extension_api::register_extension!(PerryExtension);
