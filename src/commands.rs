use std::path::{Path, PathBuf};

use zed_extension_api::{
    self as zed,
    CommandContext,
    editor::{EditorState, Range as ZedRange, TextEdit},
    ui::NotificationLevel,
};

use crate::bencode::{self, Value, dict};
use crate::forms;
use crate::jack_in::{self, JackInCommand, ProjectKind};
use crate::nrepl::{self, RequestKind, TestKind};
use crate::paredit;
use crate::PerryExtension;
use crate::tests;

pub fn run_command(
    ext: &mut PerryExtension,
    command_id: &str,
    context: CommandContext,
) -> Result<(), String> {
    let editor = context.editor.or_else(zed::editor::active_editor);

    match command_id {
        "jack-in" => handle_jack_in(ext, editor.as_ref()),
        "load-file" => handle_load_file(ext, editor.as_ref()),
        "switch-ns" => handle_switch_ns(ext, editor.as_ref()),
        "show-doc" => handle_show_doc(ext, editor.as_ref()),
        "run-ns-tests" => handle_run_ns_tests(ext, editor.as_ref()),
        "run-test-at-cursor" => handle_run_test_at_cursor(ext, editor.as_ref()),
        "run-all-tests" => handle_run_all_tests(ext, editor.as_ref()),
        "rerun-failed-tests" => handle_rerun_failed_tests(ext, editor.as_ref()),
        "clear-test-markers" => handle_clear_test_markers(editor.as_ref()),
        "paredit-slurp-forward" => handle_paredit(editor.as_ref(), |text, offset| {
            paredit::slurp_forwards(text, offset)
        }),
        "paredit-barf-forward" => handle_paredit(editor.as_ref(), |text, offset| {
            paredit::barf_forwards(text, offset)
        }),
        "paredit-slurp-backward" => handle_paredit(editor.as_ref(), |text, offset| {
            paredit::slurp_backwards(text, offset)
        }),
        "paredit-barf-backward" => handle_paredit(editor.as_ref(), |text, offset| {
            paredit::barf_backwards(text, offset)
        }),
        "paredit-raise" => handle_paredit(editor.as_ref(), |text, offset| {
            paredit::raise(text, offset)
        }),
        "paredit-splice" => handle_paredit(editor.as_ref(), |text, offset| {
            paredit::splice(text, offset)
        }),
        "paredit-splice-killing-forward" | "paredit-splice-killing-forwards" => {
            handle_paredit(editor.as_ref(), paredit::splice_killing_forwards)
        }
        "paredit-splice-killing-backward" | "paredit-splice-killing-backwards" => {
            handle_paredit(editor.as_ref(), paredit::splice_killing_backwards)
        }
        "paredit-wrap-round" => handle_paredit_wrap(editor.as_ref(), "(", ")"),
        "paredit-wrap-square" => handle_paredit_wrap(editor.as_ref(), "[", "]"),
        "paredit-wrap-curly" => handle_paredit_wrap(editor.as_ref(), "{", "}"),
        "paredit-wrap-quote" => handle_paredit_wrap(editor.as_ref(), "\"", "\""),
        "paredit-split" => handle_paredit(editor.as_ref(), |text, offset| {
            paredit::split(text, offset)
        }),
        "paredit-join" => handle_paredit(editor.as_ref(), |text, offset| {
            paredit::join(text, offset)
        }),
        "paredit-kill" => handle_paredit_kill(editor.as_ref(), paredit::kill),
        "paredit-kill-sexp" => handle_paredit_kill(editor.as_ref(), paredit::kill_sexp),
        "paredit-close-and-newline" => {
            handle_paredit(editor.as_ref(), |text, offset| {
                paredit::close_and_newline(text, offset, None)
            })
        }
        "paredit-move-forward" => handle_paredit_move(editor.as_ref(), paredit::move_forward),
        "paredit-move-backward" => handle_paredit_move(editor.as_ref(), paredit::move_backward),
        "paredit-move-up" | "paredit-move-forward-up" => {
            handle_paredit_move(editor.as_ref(), paredit::move_forward_up)
        }
        "paredit-move-down" | "paredit-move-forward-down" => {
            handle_paredit_move(editor.as_ref(), paredit::move_forward_down)
        }
        "paredit-move-backward-up" => {
            handle_paredit_move(editor.as_ref(), paredit::move_backward_up)
        }
        "paredit-move-backward-down" => {
            handle_paredit_move(editor.as_ref(), paredit::move_backward_down)
        }
        "paredit-thread-first" => handle_paredit(editor.as_ref(), paredit::thread_first),
        "paredit-thread-last" => handle_paredit(editor.as_ref(), paredit::thread_last),
        "paredit-unthread" => handle_paredit(editor.as_ref(), paredit::unthread),
        "paredit-cycle-collection" | "paredit-cycle-collection-type" => {
            handle_paredit(editor.as_ref(), paredit::cycle_collection_type)
        }
        "paredit-move-form-up" => handle_paredit(editor.as_ref(), paredit::move_form_up),
        "paredit-move-form-down" => handle_paredit(editor.as_ref(), paredit::move_form_down),
        "paredit-balanced-backspace" => handle_balanced_backspace(editor.as_ref()),
        "paredit-balanced-delete" => handle_balanced_delete(editor.as_ref()),
        "paredit-close-delimiter" => handle_paredit(editor.as_ref(), |text, offset| {
            paredit::close_delimiter(text, offset, ')')
                .or_else(|| paredit::close_delimiter(text, offset, ']'))
                .or_else(|| paredit::close_delimiter(text, offset, '}'))
        }),
        "paredit-insert-quote" | "paredit-insert-double-quote" => {
            handle_paredit(editor.as_ref(), |text, offset| {
                Some(paredit::insert_double_quote(text, offset))
            })
        }
        "paredit-comment-shunting" => handle_paredit(editor.as_ref(), |text, offset| {
            Some(paredit::comment_shunting(text, offset))
        }),
        _ => Err(format!("Unknown Perry command: {command_id}")),
    }
}

fn handle_jack_in(ext: &mut PerryExtension, _editor: Option<&EditorState>) -> Result<(), String> {
    let root_str = zed::panel::active_worktree_root().unwrap_or_else(|_| ".".to_string());
    let project_root = PathBuf::from(&root_str);
    let kind = jack_in::detect_project_kind().unwrap_or(ProjectKind::ClojureCli);

    let jack_cmd = JackInCommand::new(kind, &project_root, 7888);

    let args_slices: Vec<&str> = jack_cmd.args.iter().map(|s| s.as_str()).collect();
    let handle = zed::process::spawn(
        &jack_cmd.program,
        &args_slices,
        &[],
        Some(&root_str),
    )?;

    ext.jack_in_handle = Some(handle);
    ext.jack_in_buffer.clear();
    // Jack-in notifies "nREPL listo en el puerto N: usa repl: refresh kernelspecs" on port detection
    zed::ui::notify(
        NotificationLevel::Info,
        &format!("Jack-in started ({})", kind.label()),
    );
    Ok(())
}

fn handle_load_file(ext: &mut PerryExtension, editor: Option<&EditorState>) -> Result<(), String> {
    let editor = editor.ok_or("No active editor to load file")?;
    ext.ensure_connected()?;
    let stream = ext.tcp_stream.as_ref().ok_or("nREPL is not connected")?;
    let nrepl = ext.nrepl.as_mut().ok_or("nREPL client is not initialized")?;

    let file_name = Path::new(&editor.path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "file.clj".to_string());

    let req_id = nrepl::next_request_id();
    nrepl.register_request(
        req_id.clone(),
        RequestKind::LoadFile {
            path: editor.path.clone(),
        },
    );

    let req = nrepl::load_file_request(
        &req_id,
        nrepl.session_id.as_deref(),
        &editor.text,
        &editor.path,
        &file_name,
    );
    stream.write(&bencode::encode(&req))?;
    Ok(())
}

fn handle_switch_ns(ext: &mut PerryExtension, editor: Option<&EditorState>) -> Result<(), String> {
    let editor = editor.ok_or("No active editor to switch namespace")?;
    ext.ensure_connected()?;
    let stream = ext.tcp_stream.as_ref().ok_or("nREPL is not connected")?;
    let nrepl = ext.nrepl.as_mut().ok_or("nREPL client is not initialized")?;

    let ns = forms::parse_namespace(&editor.text).unwrap_or_else(|| "user".to_string());
    let req_id = nrepl::next_request_id();
    nrepl.register_request(req_id.clone(), RequestKind::SwitchNs { ns: ns.clone() });

    let switch_code = format!("(in-ns '{ns})");
    let req = nrepl::eval_request(
        &req_id,
        nrepl.session_id.as_deref(),
        None,
        &switch_code,
    );
    stream.write(&bencode::encode(&req))?;
    Ok(())
}

fn handle_show_doc(ext: &mut PerryExtension, editor: Option<&EditorState>) -> Result<(), String> {
    let editor = editor.ok_or("No active editor to inspect symbol")?;
    let cursor = editor.selections.first().map(|s| s.start as usize).unwrap_or(0);
    let sym = forms::symbol_at_offset(&editor.text, cursor).ok_or("No Clojure symbol at cursor")?;

    ext.ensure_connected()?;
    let stream = ext.tcp_stream.as_ref().ok_or("nREPL is not connected")?;
    let nrepl = ext.nrepl.as_mut().ok_or("nREPL client is not initialized")?;

    let ns = forms::parse_namespace(&editor.text).unwrap_or_else(|| nrepl.current_ns.clone());
    let req_id = nrepl::next_request_id();
    nrepl.register_request(req_id.clone(), RequestKind::Info { symbol: sym.clone() });

    let req = nrepl::info_request(&req_id, nrepl.session_id.as_deref(), &ns, &sym);
    stream.write(&bencode::encode(&req))?;
    Ok(())
}

fn handle_run_ns_tests(
    ext: &mut PerryExtension,
    editor: Option<&EditorState>,
) -> Result<(), String> {
    let editor = editor.ok_or("No active editor to run namespace tests")?;
    ext.ensure_connected()?;
    let stream = ext.tcp_stream.as_ref().ok_or("nREPL is not connected")?;
    let nrepl = ext.nrepl.as_mut().ok_or("nREPL client is not initialized")?;
    let session = nrepl.session_id.clone().ok_or("No active nREPL session")?;

    let ns = forms::parse_namespace(&editor.text).unwrap_or_else(|| "user".to_string());
    let query_namespaces = if !ns.ends_with("-test") {
        vec![Value::str(&ns), Value::str(format!("{ns}-test"))]
    } else {
        vec![Value::str(&ns)]
    };

    let var_query = dict([(
        "ns-query",
        dict([("exactly", Value::List(query_namespaces))]),
    )]);

    let req_id = nrepl::next_request_id();
    nrepl.register_request(
        req_id.clone(),
        RequestKind::TestVarQuery {
            kind: TestKind::Namespace { ns },
            path: Some(editor.path.clone()),
        },
    );

    let req = nrepl::test_var_query_request(&req_id, &session, var_query);
    stream.write(&bencode::encode(&req))?;
    Ok(())
}

fn handle_run_test_at_cursor(
    ext: &mut PerryExtension,
    editor: Option<&EditorState>,
) -> Result<(), String> {
    let editor = editor.ok_or("No active editor to run test at cursor")?;
    let cursor = editor.selections.first().map(|s| s.start as usize).unwrap_or(0);
    let top_level_range = forms::text_top_level_form(&editor.text, cursor)
        .ok_or("No top-level form at cursor")?;
    let top_level_text = editor.text.get(top_level_range).ok_or("Invalid form range")?;
    let var_name = tests::parse_deftest_name(top_level_text)
        .ok_or("Form at cursor is not a deftest")?;

    ext.ensure_connected()?;
    let stream = ext.tcp_stream.as_ref().ok_or("nREPL is not connected")?;
    let nrepl = ext.nrepl.as_mut().ok_or("nREPL client is not initialized")?;
    let session = nrepl.session_id.clone().ok_or("No active nREPL session")?;

    let ns = forms::parse_namespace(&editor.text).unwrap_or_else(|| "user".to_string());
    let full_var = format!("{ns}/{var_name}");

    let var_query = dict([("exact-vars", Value::List(vec![Value::str(&full_var)]))]);

    let req_id = nrepl::next_request_id();
    nrepl.register_request(
        req_id.clone(),
        RequestKind::TestVarQuery {
            kind: TestKind::TestAtCursor { var: full_var },
            path: Some(editor.path.clone()),
        },
    );

    let req = nrepl::test_var_query_request(&req_id, &session, var_query);
    stream.write(&bencode::encode(&req))?;
    Ok(())
}

fn handle_run_all_tests(
    ext: &mut PerryExtension,
    editor: Option<&EditorState>,
) -> Result<(), String> {
    ext.ensure_connected()?;
    let stream = ext.tcp_stream.as_ref().ok_or("nREPL is not connected")?;
    let nrepl = ext.nrepl.as_mut().ok_or("nREPL client is not initialized")?;
    let session = nrepl.session_id.clone().ok_or("No active nREPL session")?;

    let var_query = dict(Vec::<(&str, Value)>::new());
    let req_id = nrepl::next_request_id();
    nrepl.register_request(
        req_id.clone(),
        RequestKind::TestVarQuery {
            kind: TestKind::All,
            path: editor.map(|e| e.path.clone()),
        },
    );

    let req = nrepl::test_var_query_request(&req_id, &session, var_query);
    stream.write(&bencode::encode(&req))?;
    Ok(())
}

fn handle_rerun_failed_tests(
    ext: &mut PerryExtension,
    editor: Option<&EditorState>,
) -> Result<(), String> {
    ext.ensure_connected()?;
    let stream = ext.tcp_stream.as_ref().ok_or("nREPL is not connected")?;
    let nrepl = ext.nrepl.as_mut().ok_or("nREPL client is not initialized")?;
    let session = nrepl.session_id.clone().ok_or("No active nREPL session")?;

    let req_id = nrepl::next_request_id();
    nrepl.register_request(
        req_id.clone(),
        RequestKind::TestVarQuery {
            kind: TestKind::All,
            path: editor.map(|e| e.path.clone()),
        },
    );

    let req = dict([
        ("op", Value::str("retest")),
        ("id", Value::str(&req_id)),
        ("session", Value::str(&session)),
    ]);
    stream.write(&bencode::encode(&req))?;
    Ok(())
}

fn handle_clear_test_markers(editor: Option<&EditorState>) -> Result<(), String> {
    let editor = editor.ok_or("No active editor to clear test markers")?;
    zed::editor::clear_decorations(Some(&editor.path), "perry-tests")
}

fn handle_paredit<F>(editor: Option<&EditorState>, edit_fn: F) -> Result<(), String>
where
    F: FnOnce(&str, usize) -> Option<paredit::EditResult>,
{
    let editor = editor.ok_or("No active editor for Paredit operation")?;
    let cursor = editor.selections.first().map(|s| s.start as usize).unwrap_or(0);
    let edit = edit_fn(&editor.text, cursor).ok_or("Paredit operation not applicable")?;

    zed::editor::apply_edits(
        &editor.path,
        vec![TextEdit {
            start: edit.replacement_range.start as u32,
            end: edit.replacement_range.end as u32,
            text: edit.replacement_text,
        }],
    )?;

    zed::editor::set_selections(
        &editor.path,
        vec![ZedRange {
            start: edit.new_cursor as u32,
            end: edit.new_cursor as u32,
        }],
    )?;
    Ok(())
}

fn handle_paredit_wrap(
    editor: Option<&EditorState>,
    open: &str,
    close: &str,
) -> Result<(), String> {
    let editor = editor.ok_or("No active editor for Paredit wrap")?;
    let sel = editor.selections.first().ok_or("No cursor selection")?;
    let range = sel.start as usize..sel.end as usize;
    let edit = paredit::wrap(&editor.text, range, open, close)
        .ok_or("Cannot wrap at current position")?;

    zed::editor::apply_edits(
        &editor.path,
        vec![TextEdit {
            start: edit.replacement_range.start as u32,
            end: edit.replacement_range.end as u32,
            text: edit.replacement_text,
        }],
    )?;

    zed::editor::set_selections(
        &editor.path,
        vec![ZedRange {
            start: edit.new_cursor as u32,
            end: edit.new_cursor as u32,
        }],
    )?;
    Ok(())
}

fn handle_paredit_kill<F>(editor: Option<&EditorState>, kill_fn: F) -> Result<(), String>
where
    F: FnOnce(&str, usize) -> Option<(paredit::EditResult, String)>,
{
    let editor = editor.ok_or("No active editor for Paredit kill")?;
    let cursor = editor.selections.first().map(|s| s.start as usize).unwrap_or(0);
    let (edit, _killed) = kill_fn(&editor.text, cursor).ok_or("Paredit kill not applicable")?;

    zed::editor::apply_edits(
        &editor.path,
        vec![TextEdit {
            start: edit.replacement_range.start as u32,
            end: edit.replacement_range.end as u32,
            text: edit.replacement_text,
        }],
    )?;

    zed::editor::set_selections(
        &editor.path,
        vec![ZedRange {
            start: edit.new_cursor as u32,
            end: edit.new_cursor as u32,
        }],
    )?;
    Ok(())
}

fn handle_paredit_move<F>(editor: Option<&EditorState>, move_fn: F) -> Result<(), String>
where
    F: FnOnce(&str, usize) -> Option<usize>,
{
    let editor = editor.ok_or("No active editor for Paredit movement")?;
    let cursor = editor.selections.first().map(|s| s.start as usize).unwrap_or(0);
    let new_cursor = move_fn(&editor.text, cursor).ok_or("Paredit movement not applicable")?;

    zed::editor::set_selections(
        &editor.path,
        vec![ZedRange {
            start: new_cursor as u32,
            end: new_cursor as u32,
        }],
    )?;
    Ok(())
}

fn handle_balanced_backspace(editor: Option<&EditorState>) -> Result<(), String> {
    let editor = editor.ok_or("No active editor for Paredit balanced backspace")?;
    let has_multiple_or_selection = editor.selections.len() > 1
        || editor
            .selections
            .first()
            .map(|s| s.start != s.end)
            .unwrap_or(false);

    if !has_multiple_or_selection {
        let cursor = editor.selections.first().map(|s| s.start as usize).unwrap_or(0);
        if let Some(edit) = paredit::balanced_backspace(&editor.text, cursor) {
            if edit.replacement_range.start != edit.replacement_range.end
                || !edit.replacement_text.is_empty()
            {
                zed::editor::apply_edits(
                    &editor.path,
                    vec![TextEdit {
                        start: edit.replacement_range.start as u32,
                        end: edit.replacement_range.end as u32,
                        text: edit.replacement_text,
                    }],
                )?;
            }
            zed::editor::set_selections(
                &editor.path,
                vec![ZedRange {
                    start: edit.new_cursor as u32,
                    end: edit.new_cursor as u32,
                }],
            )?;
            return Ok(());
        }
    }

    // Normal backspace across selection(s) or multiple cursor(s)
    let mut edits = Vec::new();
    let mut new_selections = Vec::new();

    let mut sorted_selections = editor.selections.clone();
    sorted_selections.sort_by_key(|s| s.start.min(s.end));

    let mut running_delta: isize = 0;
    for sel in sorted_selections {
        let (start, end) = (sel.start.min(sel.end) as usize, sel.start.max(sel.end) as usize);
        if start != end {
            edits.push(TextEdit {
                start: start as u32,
                end: end as u32,
                text: String::new(),
            });
            let target_cursor = (start as isize + running_delta).max(0) as u32;
            new_selections.push(ZedRange {
                start: target_cursor,
                end: target_cursor,
            });
            running_delta -= (end - start) as isize;
        } else if start > 0 {
            if let Some(ch) = editor.text.get(..start).and_then(|s| s.chars().last()) {
                let prev_offset = (start - ch.len_utf8()) as u32;
                edits.push(TextEdit {
                    start: prev_offset,
                    end: start as u32,
                    text: String::new(),
                });
                let target_cursor = (prev_offset as isize + running_delta).max(0) as u32;
                new_selections.push(ZedRange {
                    start: target_cursor,
                    end: target_cursor,
                });
                running_delta -= ch.len_utf8() as isize;
            }
        }
    }

    if !edits.is_empty() {
        zed::editor::apply_edits(&editor.path, edits)?;
        zed::editor::set_selections(&editor.path, new_selections)?;
    }
    Ok(())
}

fn handle_balanced_delete(editor: Option<&EditorState>) -> Result<(), String> {
    let editor = editor.ok_or("No active editor for Paredit balanced delete")?;
    let has_multiple_or_selection = editor.selections.len() > 1
        || editor
            .selections
            .first()
            .map(|s| s.start != s.end)
            .unwrap_or(false);

    if !has_multiple_or_selection {
        let cursor = editor.selections.first().map(|s| s.start as usize).unwrap_or(0);
        if let Some(edit) = paredit::balanced_delete(&editor.text, cursor) {
            if edit.replacement_range.start != edit.replacement_range.end
                || !edit.replacement_text.is_empty()
            {
                zed::editor::apply_edits(
                    &editor.path,
                    vec![TextEdit {
                        start: edit.replacement_range.start as u32,
                        end: edit.replacement_range.end as u32,
                        text: edit.replacement_text,
                    }],
                )?;
            }
            zed::editor::set_selections(
                &editor.path,
                vec![ZedRange {
                    start: edit.new_cursor as u32,
                    end: edit.new_cursor as u32,
                }],
            )?;
            return Ok(());
        }
    }

    // Normal forward delete across selection(s) or multiple cursor(s)
    let mut edits = Vec::new();
    let mut new_selections = Vec::new();

    let mut sorted_selections = editor.selections.clone();
    sorted_selections.sort_by_key(|s| s.start.min(s.end));

    let mut running_delta: isize = 0;
    for sel in sorted_selections {
        let (start, end) = (sel.start.min(sel.end) as usize, sel.start.max(sel.end) as usize);
        if start != end {
            edits.push(TextEdit {
                start: start as u32,
                end: end as u32,
                text: String::new(),
            });
            let target_cursor = (start as isize + running_delta).max(0) as u32;
            new_selections.push(ZedRange {
                start: target_cursor,
                end: target_cursor,
            });
            running_delta -= (end - start) as isize;
        } else if start < editor.text.len() {
            if let Some(ch) = editor.text.get(start..).and_then(|s| s.chars().next()) {
                let next_offset = (start + ch.len_utf8()) as u32;
                edits.push(TextEdit {
                    start: start as u32,
                    end: next_offset,
                    text: String::new(),
                });
                let target_cursor = (start as isize + running_delta).max(0) as u32;
                new_selections.push(ZedRange {
                    start: target_cursor,
                    end: target_cursor,
                });
                running_delta -= ch.len_utf8() as isize;
            }
        }
    }

    if !edits.is_empty() {
        zed::editor::apply_edits(&editor.path, edits)?;
        zed::editor::set_selections(&editor.path, new_selections)?;
    }
    Ok(())
}

