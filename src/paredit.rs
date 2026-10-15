use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormKind {
    List,
    Vector,
    Map,
    Set,
    Fn,
    String,
    Atom,
    Comment,
}

impl FormKind {
    pub fn is_collection(&self) -> bool {
        matches!(
            self,
            FormKind::List | FormKind::Vector | FormKind::Map | FormKind::Set | FormKind::Fn
        )
    }

    pub fn delimiters(&self) -> Option<(&'static str, &'static str)> {
        match self {
            FormKind::List => Some(("(", ")")),
            FormKind::Vector => Some(("[", "]")),
            FormKind::Map => Some(("{", "}")),
            FormKind::Set => Some(("#{", "}")),
            FormKind::Fn => Some(("#(", ")")),
            FormKind::String => Some(("\"", "\"")),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormNode {
    pub kind: FormKind,
    pub range: Range<usize>,
    pub open_len: usize,
    pub close_len: usize,
    pub children: Vec<FormNode>,
}

impl FormNode {
    pub fn inner_range(&self) -> Range<usize> {
        let start = self.range.start + self.open_len;
        let end = self.range.end.saturating_sub(self.close_len);
        start..end.max(start)
    }

    pub fn is_empty_collection(&self) -> bool {
        self.kind.is_collection() && self.children.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditResult {
    pub replacement_range: Range<usize>,
    pub replacement_text: String,
    pub new_cursor: usize,
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b',')
}

fn is_delim(b: u8) -> bool {
    matches!(b, b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'"' | b';')
}

pub fn parse_tree(text: &str) -> Vec<FormNode> {
    let bytes = text.as_bytes();
    let mut index = 0;
    parse_forms_in_range(bytes, &mut index, None)
}

fn parse_forms_in_range(
    bytes: &[u8],
    index: &mut usize,
    closing_byte: Option<u8>,
) -> Vec<FormNode> {
    let mut nodes = Vec::new();

    while *index < bytes.len() {
        let b = bytes[*index];

        if is_ws(b) {
            *index += 1;
            continue;
        }

        if Some(b) == closing_byte {
            break;
        }

        if b == b';' {
            let start = *index;
            while *index < bytes.len() && bytes[*index] != b'\n' {
                *index += 1;
            }
            nodes.push(FormNode {
                kind: FormKind::Comment,
                range: start..*index,
                open_len: 1,
                close_len: 0,
                children: Vec::new(),
            });
            continue;
        }

        if b == b'"' {
            let start = *index;
            *index += 1;
            while *index < bytes.len() {
                if bytes[*index] == b'\\' {
                    *index += 1;
                    if *index < bytes.len() {
                        *index += 1;
                    }
                } else if bytes[*index] == b'"' {
                    *index += 1;
                    break;
                } else {
                    *index += 1;
                }
            }
            nodes.push(FormNode {
                kind: FormKind::String,
                range: start..*index,
                open_len: 1,
                close_len: 1,
                children: Vec::new(),
            });
            continue;
        }

        if b == b'(' || b == b'[' || b == b'{' {
            let start = *index;
            let (expected_close, kind) = match b {
                b'(' => (b')', FormKind::List),
                b'[' => (b']', FormKind::Vector),
                b'{' => (b'}', FormKind::Map),
                _ => unreachable!(),
            };
            *index += 1;
            let children = parse_forms_in_range(bytes, index, Some(expected_close));
            let close_len = if *index < bytes.len() && bytes[*index] == expected_close {
                *index += 1;
                1
            } else {
                0
            };
            nodes.push(FormNode {
                kind,
                range: start..*index,
                open_len: 1,
                close_len,
                children,
            });
            continue;
        }

        if b == b'#' {
            let start = *index;
            *index += 1;
            if *index < bytes.len() {
                let next = bytes[*index];
                if next == b'{' {
                    *index += 1;
                    let children = parse_forms_in_range(bytes, index, Some(b'}'));
                    let close_len = if *index < bytes.len() && bytes[*index] == b'}' {
                        *index += 1;
                        1
                    } else {
                        0
                    };
                    nodes.push(FormNode {
                        kind: FormKind::Set,
                        range: start..*index,
                        open_len: 2,
                        close_len,
                        children,
                    });
                    continue;
                } else if next == b'(' {
                    *index += 1;
                    let children = parse_forms_in_range(bytes, index, Some(b')'));
                    let close_len = if *index < bytes.len() && bytes[*index] == b')' {
                        *index += 1;
                        1
                    } else {
                        0
                    };
                    nodes.push(FormNode {
                        kind: FormKind::Fn,
                        range: start..*index,
                        open_len: 2,
                        close_len,
                        children,
                    });
                    continue;
                }
            }
        }

        if matches!(b, b'\'' | b'`' | b'~' | b'@' | b'^') {
            let start = *index;
            *index += 1;
            if b == b'~' && *index < bytes.len() && bytes[*index] == b'@' {
                *index += 1;
            }
            let children = parse_forms_in_range(bytes, index, closing_byte);
            let end = children.last().map(|c| c.range.end).unwrap_or(*index);
            nodes.push(FormNode {
                kind: FormKind::Atom,
                range: start..end,
                open_len: 0,
                close_len: 0,
                children,
            });
            continue;
        }

        if b == b')' || b == b']' || b == b'}' {
            *index += 1;
            continue;
        }

        let start = *index;
        while *index < bytes.len() && !is_ws(bytes[*index]) && !is_delim(bytes[*index]) {
            *index += 1;
        }
        nodes.push(FormNode {
            kind: FormKind::Atom,
            range: start..*index,
            open_len: 0,
            close_len: 0,
            children: Vec::new(),
        });
    }

    nodes
}

pub fn find_enclosing_collection<'a>(
    nodes: &'a [FormNode],
    offset: usize,
) -> Option<(&'a FormNode, Option<&'a FormNode>)> {
    let mut current_nodes = nodes;
    let mut parent: Option<&'a FormNode> = None;
    let mut candidate: Option<&'a FormNode> = None;

    loop {
        let child_match = current_nodes
            .iter()
            .find(|n| n.kind.is_collection() && n.range.start <= offset && offset <= n.range.end);

        match child_match {
            Some(node) => {
                parent = candidate;
                candidate = Some(node);
                current_nodes = &node.children;
            }
            None => break,
        }
    }

    candidate.map(|c| (c, parent))
}

pub fn find_form_at_cursor(nodes: &[FormNode], offset: usize) -> Option<&FormNode> {
    for node in nodes {
        if node.range.contains(&offset) || node.range.end == offset {
            if !node.children.is_empty() {
                if let Some(inner) = find_form_at_cursor(&node.children, offset) {
                    return Some(inner);
                }
            }
            return Some(node);
        }
    }
    None
}

pub fn slurp_forwards(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, parent) = find_enclosing_collection(&tree, offset)?;
    let siblings = match parent {
        Some(p) => &p.children,
        None => &tree,
    };
    let coll_idx = siblings.iter().position(|s| s.range == coll.range)?;
    let next_sibling = siblings.get(coll_idx + 1)?;

    let close_delim = text.get(coll.range.end - coll.close_len..coll.range.end)?;
    let before_close = text.get(coll.range.start..coll.range.end - coll.close_len)?;
    let slurped_text = text.get(coll.range.end..next_sibling.range.end)?;

    let inner_empty = coll.children.is_empty();
    let body = before_close.trim_end();
    let slurped = slurped_text.trim();
    let replacement_text = if inner_empty {
        format!("{before_close}{slurped}{close_delim}")
    } else {
        format!("{body} {slurped}{close_delim}")
    };

    let replacement_range = coll.range.start..next_sibling.range.end;
    Some(EditResult {
        replacement_range,
        replacement_text,
        new_cursor: offset,
    })
}

pub fn slurp_backwards(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, parent) = find_enclosing_collection(&tree, offset)?;
    let siblings = match parent {
        Some(p) => &p.children,
        None => &tree,
    };
    let coll_idx = siblings.iter().position(|s| s.range == coll.range)?;
    if coll_idx == 0 {
        return None;
    }
    let prev_sibling = siblings.get(coll_idx - 1)?;

    let open_delim = text.get(coll.range.start..coll.range.start + coll.open_len)?;
    let prev_text = text.get(prev_sibling.range.start..coll.range.start)?;
    let inner_and_close = text.get(coll.range.start + coll.open_len..coll.range.end)?;

    let separator = if inner_and_close.starts_with(char::is_whitespace)
        || prev_text.ends_with(char::is_whitespace)
    {
        ""
    } else {
        " "
    };

    let replacement_text = format!("{open_delim}{prev_text}{separator}{inner_and_close}");
    let replacement_range = prev_sibling.range.start..coll.range.end;
    let shift = coll.open_len;
    Some(EditResult {
        replacement_range,
        replacement_text,
        new_cursor: offset.saturating_add(shift),
    })
}

pub fn barf_forwards(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    if coll.children.is_empty() {
        return None;
    }
    let last_child = coll.children.last()?;
    let close_delim = text.get(coll.range.end - coll.close_len..coll.range.end)?;

    let open_and_body = text.get(coll.range.start..last_child.range.start)?.trim_end();
    let expelled = text.get(last_child.range.start..coll.range.end - coll.close_len)?.trim_start();

    let replacement_text = format!("{open_and_body}{close_delim} {expelled}");
    let replacement_range = coll.range.clone();
    let new_cursor = offset.min(coll.range.start + open_and_body.len());
    Some(EditResult {
        replacement_range,
        replacement_text,
        new_cursor,
    })
}

pub fn barf_backwards(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    if coll.children.is_empty() {
        return None;
    }
    let first_child = coll.children.first()?;
    let open_delim = text.get(coll.range.start..coll.range.start + coll.open_len)?;

    let expelled = text.get(first_child.range.start..first_child.range.end)?;
    let remaining_inner = text.get(first_child.range.end..coll.range.end)?.trim_start();

    let replacement_text = format!("{expelled} {open_delim}{remaining_inner}");
    let replacement_range = coll.range.clone();
    let new_cursor = offset.max(coll.range.start + expelled.len() + 1);
    Some(EditResult {
        replacement_range,
        replacement_text,
        new_cursor,
    })
}

pub fn raise(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let target = find_form_at_cursor(&tree, offset)?;
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    if coll.range == target.range {
        return None;
    }
    let target_text = text.get(target.range.clone())?;
    Some(EditResult {
        replacement_range: coll.range.clone(),
        replacement_text: target_text.to_string(),
        new_cursor: coll.range.start,
    })
}

pub fn splice(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    let inner_range = coll.inner_range();
    let inner_text = text.get(inner_range)?;
    Some(EditResult {
        replacement_range: coll.range.clone(),
        replacement_text: inner_text.trim().to_string(),
        new_cursor: coll.range.start,
    })
}

pub fn wrap(text: &str, range: Range<usize>, open: &str, close: &str) -> Option<EditResult> {
    if !range.is_empty() {
        let selected = text.get(range.clone())?;
        let replacement_text = format!("{open}{selected}{close}");
        let new_cursor = range.start + open.len() + selected.len();
        return Some(EditResult {
            replacement_range: range,
            replacement_text,
            new_cursor,
        });
    }

    let tree = parse_tree(text);
    let target = find_form_at_cursor(&tree, range.start).or_else(|| {
        let after = text[range.start..].find(|c: char| !c.is_whitespace())?;
        find_form_at_cursor(&tree, range.start + after)
    })?;

    let form_text = text.get(target.range.clone())?;
    let replacement_text = format!("{open}{form_text}{close}");
    let new_cursor = target.range.start + open.len();
    Some(EditResult {
        replacement_range: target.range.clone(),
        replacement_text,
        new_cursor,
    })
}

pub fn split(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    let (open, close) = coll.kind.delimiters()?;

    let clamped = offset.clamp(
        coll.range.start + coll.open_len,
        coll.range.end - coll.close_len,
    );
    let left = text.get(coll.range.start + coll.open_len..clamped)?.trim_end();
    let right = text.get(clamped..coll.range.end - coll.close_len)?.trim_start();

    let replacement_text = format!("{open}{left}{close} {open}{right}{close}");
    Some(EditResult {
        replacement_range: coll.range.clone(),
        replacement_text,
        new_cursor: coll.range.start + open.len() + left.len() + close.len() + 1,
    })
}

pub fn join(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, parent) = find_enclosing_collection(&tree, offset).or_else(|| {
        find_form_at_cursor(&tree, offset)
            .filter(|f| f.kind.is_collection())
            .map(|f| (f, None))
    })?;
    let siblings = match parent {
        Some(p) => &p.children,
        None => &tree,
    };
    let idx = siblings.iter().position(|s| s.range == coll.range)?;
    let next = siblings.get(idx + 1)?;
    if coll.kind != next.kind {
        return None;
    }
    let (open, close) = coll.kind.delimiters()?;
    let left_inner =
        text.get(coll.range.start + coll.open_len..coll.range.end - coll.close_len)?.trim();
    let right_inner =
        text.get(next.range.start + next.open_len..next.range.end - next.close_len)?.trim();

    let sep = if left_inner.is_empty() || right_inner.is_empty() {
        ""
    } else {
        " "
    };
    let replacement_text = format!("{open}{left_inner}{sep}{right_inner}{close}");
    Some(EditResult {
        replacement_range: coll.range.start..next.range.end,
        replacement_text,
        new_cursor: coll.range.start + open.len() + left_inner.len(),
    })
}

pub fn splice_killing_forwards(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    let kept_end = offset.min(coll.range.end.saturating_sub(coll.close_len));
    let kept = text.get(coll.range.start + coll.open_len..kept_end)?.trim();
    Some(EditResult {
        replacement_range: coll.range.clone(),
        replacement_text: kept.to_string(),
        new_cursor: coll.range.start,
    })
}

pub fn splice_killing_backwards(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    let kept_start = offset.max(coll.range.start + coll.open_len);
    let kept = text.get(kept_start..coll.range.end.saturating_sub(coll.close_len))?.trim();
    Some(EditResult {
        replacement_range: coll.range.clone(),
        replacement_text: kept.to_string(),
        new_cursor: coll.range.start,
    })
}

pub fn kill(text: &str, offset: usize) -> Option<(EditResult, String)> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    let end_of_line = text.get(offset..)?
        .find('\n')
        .map(|p| offset + p)
        .unwrap_or(text.len());
    let cut_end = (coll.range.end.saturating_sub(coll.close_len)).min(end_of_line);
    if cut_end <= offset {
        return None;
    }
    let killed_text = text.get(offset..cut_end)?.to_string();
    Some((
        EditResult {
            replacement_range: offset..cut_end,
            replacement_text: String::new(),
            new_cursor: offset,
        },
        killed_text,
    ))
}

pub fn kill_sexp(text: &str, offset: usize) -> Option<(EditResult, String)> {
    let tree = parse_tree(text);
    let target = find_form_at_cursor(&tree, offset).or_else(|| {
        let after = text.get(offset..)?.find(|c: char| !c.is_whitespace())?;
        find_form_at_cursor(&tree, offset + after)
    })?;
    let killed_text = text.get(target.range.clone())?.to_string();
    Some((
        EditResult {
            replacement_range: target.range.clone(),
            replacement_text: String::new(),
            new_cursor: target.range.start,
        },
        killed_text,
    ))
}

pub fn copy_as_kill(text: &str, offset: usize) -> Option<String> {
    kill(text, offset).map(|(_, killed)| killed)
}

pub fn close_and_newline(
    text: &str,
    offset: usize,
    expected_delim: Option<char>,
) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    if let Some(expected) = expected_delim {
        let (_, close) = coll.kind.delimiters()?;
        if !close.starts_with(expected) {
            return None;
        }
    }
    let close_pos = coll.range.end;
    Some(EditResult {
        replacement_range: close_pos..close_pos,
        replacement_text: "\n".to_string(),
        new_cursor: close_pos + 1,
    })
}

pub fn move_forward(text: &str, offset: usize) -> Option<usize> {
    let tree = parse_tree(text);
    if let Some(target) = find_form_at_cursor(&tree, offset) {
        if target.range.end > offset {
            return Some(target.range.end);
        }
    }
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    if offset < coll.range.end {
        Some(coll.range.end)
    } else {
        None
    }
}

pub fn move_backward(text: &str, offset: usize) -> Option<usize> {
    let tree = parse_tree(text);
    if let Some(target) = find_form_at_cursor(&tree, offset.saturating_sub(1)) {
        if target.range.start < offset {
            return Some(target.range.start);
        }
    }
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    if offset > coll.range.start {
        Some(coll.range.start)
    } else {
        None
    }
}

fn collect_all_collections<'a>(nodes: &'a [FormNode], acc: &mut Vec<&'a FormNode>) {
    for node in nodes {
        if node.kind.is_collection() {
            acc.push(node);
            collect_all_collections(&node.children, acc);
        }
    }
}

pub fn move_forward_down(text: &str, offset: usize) -> Option<usize> {
    let tree = parse_tree(text);
    let mut all = Vec::new();
    collect_all_collections(&tree, &mut all);
    let target = all.into_iter().find(|n| n.range.start >= offset)?;
    Some(target.range.start + target.open_len)
}

pub fn move_backward_down(text: &str, offset: usize) -> Option<usize> {
    let tree = parse_tree(text);
    let mut all = Vec::new();
    collect_all_collections(&tree, &mut all);
    let target = all.into_iter().rfind(|n| n.range.end <= offset)?;
    Some(target.range.end.saturating_sub(target.close_len))
}

pub fn move_forward_up(text: &str, offset: usize) -> Option<usize> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    Some(coll.range.end)
}

pub fn move_backward_up(text: &str, offset: usize) -> Option<usize> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    Some(coll.range.start)
}

pub fn thread_first(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    if coll.kind != FormKind::List || coll.children.len() < 2 {
        return None;
    }
    let first = &coll.children[0];
    let second = &coll.children[1];
    let mut remaining = Vec::new();
    for c in &coll.children[2..] {
        remaining.push(text.get(c.range.clone())?.to_string());
    }

    let head_text = text.get(first.range.clone())?;
    let arg_text = text.get(second.range.clone())?;
    let rem_text = if remaining.is_empty() {
        String::new()
    } else {
        format!(" {}", remaining.join(" "))
    };

    let replacement_text = format!("(-> {arg_text} ({head_text}{rem_text}))");
    Some(EditResult {
        replacement_range: coll.range.clone(),
        replacement_text,
        new_cursor: coll.range.start,
    })
}

pub fn thread_last(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    if coll.kind != FormKind::List || coll.children.len() < 2 {
        return None;
    }
    let first = &coll.children[0];
    let last = coll.children.last()?;
    let mut middle = Vec::new();
    for c in &coll.children[1..coll.children.len() - 1] {
        middle.push(text.get(c.range.clone())?.to_string());
    }

    let head_text = text.get(first.range.clone())?;
    let arg_text = text.get(last.range.clone())?;
    let mid_text = if middle.is_empty() {
        String::new()
    } else {
        format!(" {}", middle.join(" "))
    };

    let replacement_text = format!("(->> {arg_text} ({head_text}{mid_text}))");
    Some(EditResult {
        replacement_range: coll.range.clone(),
        replacement_text,
        new_cursor: coll.range.start,
    })
}

pub fn unthread(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    if coll.kind != FormKind::List || coll.children.len() < 3 {
        return None;
    }
    let macro_sym = text.get(coll.children[0].range.clone())?;
    let is_last = macro_sym == "->>";
    if macro_sym != "->" && !is_last {
        return None;
    }

    let mut current = text.get(coll.children[1].range.clone())?.to_string();
    for step in &coll.children[2..] {
        if step.kind == FormKind::List && !step.children.is_empty() {
            let fn_name = text.get(step.children[0].range.clone())?;
            let mut rest_args = Vec::new();
            for c in &step.children[1..] {
                rest_args.push(text.get(c.range.clone())?);
            }
            if is_last {
                let rest_str = if rest_args.is_empty() {
                    String::new()
                } else {
                    format!(" {}", rest_args.join(" "))
                };
                current = format!("({fn_name}{rest_str} {current})");
            } else {
                let rest_str = if rest_args.is_empty() {
                    String::new()
                } else {
                    format!(" {}", rest_args.join(" "))
                };
                current = format!("({fn_name} {current}{rest_str})");
            }
        } else {
            let fn_name = text.get(step.range.clone())?;
            current = format!("({fn_name} {current})");
        }
    }

    Some(EditResult {
        replacement_range: coll.range.clone(),
        replacement_text: current,
        new_cursor: coll.range.start,
    })
}

pub fn cycle_collection_type(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    let (next_open, next_close) = match coll.kind {
        FormKind::List => ("[", "]"),
        FormKind::Vector => ("{", "}"),
        FormKind::Map => ("#{", "}"),
        FormKind::Set => ("(", ")"),
        _ => return None,
    };
    let inner = text.get(coll.range.start + coll.open_len..coll.range.end.saturating_sub(coll.close_len))?;
    let replacement_text = format!("{next_open}{inner}{next_close}");
    Some(EditResult {
        replacement_range: coll.range.clone(),
        replacement_text,
        new_cursor: offset,
    })
}

pub fn move_form_up(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let target = find_form_at_cursor(&tree, offset)?;
    let siblings = if let Some((coll, _)) = find_enclosing_collection(&tree, offset) {
        &coll.children
    } else {
        &tree
    };
    let idx = siblings.iter().position(|s| s.range == target.range)?;
    if idx == 0 {
        return None;
    }
    let prev = &siblings[idx - 1];
    let prev_text = text.get(prev.range.clone())?;
    let target_text = text.get(target.range.clone())?;

    let replacement_text = format!("{target_text} {prev_text}");
    Some(EditResult {
        replacement_range: prev.range.start..target.range.end,
        replacement_text,
        new_cursor: prev.range.start,
    })
}

pub fn move_form_down(text: &str, offset: usize) -> Option<EditResult> {
    let tree = parse_tree(text);
    let target = find_form_at_cursor(&tree, offset)?;
    let siblings = if let Some((coll, _)) = find_enclosing_collection(&tree, offset) {
        &coll.children
    } else {
        &tree
    };
    let idx = siblings.iter().position(|s| s.range == target.range)?;
    if idx + 1 >= siblings.len() {
        return None;
    }
    let next = &siblings[idx + 1];
    let target_text = text.get(target.range.clone())?;
    let next_text = text.get(next.range.clone())?;

    let replacement_text = format!("{next_text} {target_text}");
    Some(EditResult {
        replacement_range: target.range.start..next.range.end,
        replacement_text,
        new_cursor: target.range.start + next_text.len() + 1,
    })
}

pub fn balanced_backspace(text: &str, offset: usize) -> Option<EditResult> {
    if offset == 0 || offset > text.len() {
        return None;
    }
    let prev_char = text.get(..offset)?.chars().last()?;
    let next_char = text.get(offset..)?.chars().next();

    // Check empty pair: (|), [|], {|}, "|"
    if matches!(
        (prev_char, next_char),
        ('(', Some(')')) | ('[', Some(']')) | ('{', Some('}')) | ('"', Some('"'))
    ) {
        let prev_len = prev_char.len_utf8();
        let next_len = next_char.map(|c| c.len_utf8()).unwrap_or(0);
        return Some(EditResult {
            replacement_range: offset - prev_len..offset + next_len,
            replacement_text: String::new(),
            new_cursor: offset - prev_len,
        });
    }

    // After closing delimiter: jump over it to keep balance
    if matches!(prev_char, ')' | ']' | '}') {
        return Some(EditResult {
            replacement_range: offset..offset,
            replacement_text: String::new(),
            new_cursor: offset - prev_char.len_utf8(),
        });
    }

    // Regular backspace
    let prev_len = prev_char.len_utf8();
    Some(EditResult {
        replacement_range: offset - prev_len..offset,
        replacement_text: String::new(),
        new_cursor: offset - prev_len,
    })
}

pub fn balanced_delete(text: &str, offset: usize) -> Option<EditResult> {
    if offset >= text.len() {
        return None;
    }
    let next_char = text.get(offset..)?.chars().next()?;
    let prev_char = text.get(..offset)?.chars().last();

    // Right before empty pair: |(), |[], |{}, |""
    if offset + 2 <= text.len() {
        if let Some(pair) = text.get(offset..offset + 2) {
            if matches!(pair, "()" | "[]" | "{}" | "\"\"") {
                return Some(EditResult {
                    replacement_range: offset..offset + 2,
                    replacement_text: String::new(),
                    new_cursor: offset,
                });
            }
        }
    }

    // Check empty pair around cursor
    if matches!(
        (prev_char, next_char),
        (Some('('), ')') | (Some('['), ']') | (Some('{'), '}') | (Some('"'), '"')
    ) {
        let prev_len = prev_char.map(|c| c.len_utf8()).unwrap_or(0);
        let next_len = next_char.len_utf8();
        return Some(EditResult {
            replacement_range: offset - prev_len..offset + next_len,
            replacement_text: String::new(),
            new_cursor: offset - prev_len,
        });
    }

    // Before opening delimiter: jump inside to maintain balance
    if matches!(next_char, '(' | '[' | '{') {
        return Some(EditResult {
            replacement_range: offset..offset,
            replacement_text: String::new(),
            new_cursor: offset + next_char.len_utf8(),
        });
    }

    let next_len = next_char.len_utf8();
    Some(EditResult {
        replacement_range: offset..offset + next_len,
        replacement_text: String::new(),
        new_cursor: offset,
    })
}

pub fn close_delimiter(text: &str, offset: usize, ch: char) -> Option<EditResult> {
    let tree = parse_tree(text);
    let (coll, _) = find_enclosing_collection(&tree, offset)?;
    let (_, close) = coll.kind.delimiters()?;
    if !close.starts_with(ch) {
        return None;
    }
    let close_idx = coll.range.end.saturating_sub(coll.close_len);
    if offset <= close_idx {
        let ws_range = offset..close_idx;
        if text.get(ws_range.clone())?.trim().is_empty() {
            return Some(EditResult {
                replacement_range: ws_range,
                replacement_text: String::new(),
                new_cursor: coll.range.end,
            });
        }
    }
    Some(EditResult {
        replacement_range: offset..offset,
        replacement_text: String::new(),
        new_cursor: coll.range.end,
    })
}

pub fn insert_double_quote(text: &str, offset: usize) -> EditResult {
    let tree = parse_tree(text);
    if let Some(target) = find_form_at_cursor(&tree, offset) {
        if target.kind == FormKind::String
            && target.range.start < offset
            && offset < target.range.end
        {
            return EditResult {
                replacement_range: offset..offset,
                replacement_text: "\\\"".to_string(),
                new_cursor: offset + 2,
            };
        }
    }
    EditResult {
        replacement_range: offset..offset,
        replacement_text: "\"\"".to_string(),
        new_cursor: offset + 1,
    }
}

pub fn comment_shunting(text: &str, offset: usize) -> EditResult {
    let line_end = text.get(offset..)
        .and_then(|s| s.find('\n'))
        .map(|p| offset + p)
        .unwrap_or(text.len());
    let rest_of_line = text.get(offset..line_end).unwrap_or("");
    let trimmed = rest_of_line.trim();

    if !trimmed.is_empty() && !trimmed.starts_with(';') {
        let replacement_text = format!(";\n  {trimmed}");
        EditResult {
            replacement_range: offset..line_end,
            replacement_text,
            new_cursor: offset + 1,
        }
    } else {
        EditResult {
            replacement_range: offset..offset,
            replacement_text: ";".to_string(),
            new_cursor: offset + 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slurp_forwards() {
        assert_eq!(
            slurp_forwards("(a b) c", 2),
            Some(EditResult {
                replacement_range: 0..7,
                replacement_text: "(a b c)".to_string(),
                new_cursor: 2,
            })
        );
        assert_eq!(
            slurp_forwards("() c", 1),
            Some(EditResult {
                replacement_range: 0..4,
                replacement_text: "(c)".to_string(),
                new_cursor: 1,
            })
        );
    }

    #[test]
    fn test_slurp_backwards() {
        let res = slurp_backwards("a (b c)", 3);
        assert!(res.is_some());
        let res = res.unwrap();
        assert_eq!(res.replacement_range, 0..7);
        assert_eq!(res.replacement_text, "(a b c)");
    }

    #[test]
    fn test_barf_forwards() {
        assert_eq!(
            barf_forwards("(a b c)", 2),
            Some(EditResult {
                replacement_range: 0..7,
                replacement_text: "(a b) c".to_string(),
                new_cursor: 2,
            })
        );
    }

    #[test]
    fn test_barf_backwards() {
        let res = barf_backwards("(a b c)", 3);
        assert!(res.is_some());
        let res = res.unwrap();
        assert_eq!(res.replacement_range, 0..7);
        assert_eq!(res.replacement_text, "a (b c)");
    }

    #[test]
    fn test_raise() {
        assert_eq!(
            raise("(a (b c) d)", 6),
            Some(EditResult {
                replacement_range: 3..8,
                replacement_text: "c".to_string(),
                new_cursor: 3,
            })
        );
        assert_eq!(
            raise("(a (b c) d)", 4),
            Some(EditResult {
                replacement_range: 3..8,
                replacement_text: "b".to_string(),
                new_cursor: 3,
            })
        );
    }

    #[test]
    fn test_splice() {
        assert_eq!(
            splice("(a (b c) d)", 4),
            Some(EditResult {
                replacement_range: 3..8,
                replacement_text: "b c".to_string(),
                new_cursor: 3,
            })
        );
    }

    #[test]
    fn test_split_and_join() {
        let split_res = split("(a b c d)", 4).unwrap();
        assert_eq!(split_res.replacement_range, 0..9);
        assert_eq!(split_res.replacement_text, "(a b) (c d)");

        let join_res = join("(a b) (c d)", 2).unwrap();
        assert_eq!(join_res.replacement_range, 0..11);
        assert_eq!(join_res.replacement_text, "(a b c d)");
    }

    #[test]
    fn test_kill_and_copy() {
        let (edit, killed) = kill("(a b c d)", 3).unwrap();
        assert_eq!(killed, "b c d");
        assert_eq!(edit.replacement_range, 3..8);

        let copied = copy_as_kill("(a b c d)", 3);
        assert_eq!(copied, Some("b c d".to_string()));
    }

    #[test]
    fn test_wrap() {
        let res = wrap("foo", 0..0, "(", ")").unwrap();
        assert_eq!(res.replacement_range, 0..3);
        assert_eq!(res.replacement_text, "(foo)");

        let res_sel = wrap("foo bar", 0..3, "[", "]").unwrap();
        assert_eq!(res_sel.replacement_range, 0..3);
        assert_eq!(res_sel.replacement_text, "[foo]");
    }

    #[test]
    fn test_thread_and_unthread() {
        let thread_res = thread_first("(f x a b)", 2).unwrap();
        assert_eq!(thread_res.replacement_text, "(-> x (f a b))");

        let thread_last_res = thread_last("(f a b x)", 2).unwrap();
        assert_eq!(thread_last_res.replacement_text, "(->> x (f a b))");

        let unthread_res = unthread("(-> x (f a) (g b))", 2).unwrap();
        assert_eq!(unthread_res.replacement_text, "(g (f x a) b)");
    }

    #[test]
    fn test_cycle_collection_type() {
        let res = cycle_collection_type("(1 2 3)", 2).unwrap();
        assert_eq!(res.replacement_text, "[1 2 3]");
        let res2 = cycle_collection_type("[1 2 3]", 2).unwrap();
        assert_eq!(res2.replacement_text, "{1 2 3}");
    }

    #[test]
    fn test_balanced_backspace_and_delete() {
        let bs = balanced_backspace("()", 1).unwrap();
        assert_eq!(bs.replacement_range, 0..2);
        assert_eq!(bs.replacement_text, "");

        let del = balanced_delete("()", 0).unwrap();
        assert_eq!(del.replacement_range, 0..2);
        assert_eq!(del.replacement_text, "");

        let jump = balanced_backspace("(a b)", 5).unwrap();
        assert_eq!(jump.new_cursor, 4);
    }

    #[test]
    fn test_insert_quote_and_comment_shunting() {
        let q = insert_double_quote("\"hello \"", 7);
        assert_eq!(q.replacement_text, "\\\"");

        let cs = comment_shunting("(foo) (bar)", 5);
        assert_eq!(cs.replacement_text, ";\n  (bar)");
    }

    #[test]
    fn test_splice_killing() {
        let kf = splice_killing_forwards("(a (b c d) e)", 5).unwrap();
        assert_eq!(kf.replacement_text, "b");

        let kb = splice_killing_backwards("(a (b c d) e)", 5).unwrap();
        assert_eq!(kb.replacement_text, "c d");
    }

    #[test]
    fn test_movement() {
        assert_eq!(move_forward("(a b c)", 1), Some(2));
        assert_eq!(move_backward("(a b c)", 3), Some(1));

        assert_eq!(move_forward_down("(a [b c])", 1), Some(4));
        assert_eq!(move_backward_down("(a [b c])", 8), Some(7));

        assert_eq!(move_forward_up("(a b c)", 2), Some(7));
        assert_eq!(move_backward_up("(a b c)", 2), Some(0));
    }

    #[test]
    fn test_move_form_up_and_down() {
        let up = move_form_up("(a b c)", 3).unwrap();
        assert_eq!(up.replacement_text, "b a");

        let down = move_form_down("(a b c)", 3).unwrap();
        assert_eq!(down.replacement_text, "c b");
    }

    #[test]
    fn test_close_and_newline() {
        let cl = close_and_newline("(defn f [x] (+ x 1))", 12, None).unwrap();
        assert_eq!(cl.replacement_text, "\n");
        assert_eq!(cl.replacement_range, 19..19);
    }

    #[test]
    fn test_close_delimiter() {
        let cd = close_delimiter("(foo bar   )", 8, ')').unwrap();
        assert_eq!(cd.new_cursor, 12);
        assert_eq!(cd.replacement_text, "");
    }

    #[test]
    fn test_kill_sexp() {
        let (res, killed) = kill_sexp("(a b c)", 3).unwrap();
        assert_eq!(killed, "b");
        assert_eq!(res.replacement_range, 3..4);
    }
}
