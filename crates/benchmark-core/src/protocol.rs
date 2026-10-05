//! The structured model-response protocol and its recovery logic.
//!
//! Models are asked for one JSON object:
//! `{"changes":[{"path":"...","content":"..."}],"explanation":"..."}`.
//! Real models often wrap it in markdown, add prose, forget to escape newlines or run out of
//! tokens. This module recovers what it safely can, and records *how* it had to recover so the
//! reliability component of the score reflects it.

use pulsebench_sandbox::normalize_rel_path;
use pulsebench_types::{AppliedChange, ChangeStatus, ParseMode, ParseReport};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct ProposedChange {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct ParsedResponse {
    pub changes: Vec<ProposedChange>,
    pub report: ParseReport,
}

const PATH_KEYS: &[&str] = &["path", "file", "filename", "file_path", "filepath"];
const CONTENT_KEYS: &[&str] = &["content", "contents", "code", "new_content"];

pub fn parse_response(raw: &str, editable: &[String]) -> ParsedResponse {
    let mut notes: Vec<String> = Vec::new();
    let mut text = raw.trim().to_string();

    let stripped = strip_think_blocks(&text);
    if stripped != text {
        notes.push("removed <think> reasoning block".into());
        text = stripped.trim().to_string();
    }
    if text.is_empty() {
        notes.push("the response was empty".into());
        return failed(notes);
    }

    // 1. Clean JSON.
    if let Some(v) = parse_json_object(&text) {
        if let Some((changes, explanation, key_notes)) = extract_changes(&v) {
            let clean = key_notes.is_empty() && notes.is_empty();
            notes.extend(key_notes);
            return ok(changes, explanation, if clean { ParseMode::Json } else { ParseMode::Recovered }, notes);
        }
    }

    // 2. JSON inside a markdown fence.
    for block in fenced_blocks(&text) {
        if matches!(block.lang.as_str(), "json" | "" | "jsonc") {
            if let Some((v, repaired)) = parse_json_lenient(&block.body) {
                if let Some((changes, explanation, key_notes)) = extract_changes(&v) {
                    notes.push("JSON was wrapped in a markdown code fence".into());
                    if repaired {
                        notes.push("repaired unescaped control characters / trailing commas".into());
                    }
                    notes.extend(key_notes);
                    return ok(changes, explanation, ParseMode::Recovered, notes);
                }
            }
        }
    }

    // 3. A balanced JSON object somewhere in surrounding prose.
    for candidate in balanced_objects(&text) {
        if let Some((v, repaired)) = parse_json_lenient(&candidate) {
            if let Some((changes, explanation, key_notes)) = extract_changes(&v) {
                notes.push("JSON object extracted from surrounding text".into());
                if repaired {
                    notes.push("repaired unescaped control characters / trailing commas".into());
                }
                notes.extend(key_notes);
                return ok(changes, explanation, ParseMode::Recovered, notes);
            }
        }
    }

    // 4. Markdown code blocks mapped to editable files.
    let md = changes_from_markdown(&text, editable);
    if !md.is_empty() {
        notes.push("no valid JSON found; files recovered from markdown code blocks".into());
        return ok(md, None, ParseMode::Recovered, notes);
    }

    if looks_truncated(&text) {
        notes.push("output appears truncated: a JSON object was opened but never closed (raise the output token limit?)".into());
    } else {
        notes.push("no JSON object with a `changes` array and no recognizable code blocks were found".into());
    }
    failed(notes)
}

fn ok(changes: Vec<ProposedChange>, explanation: Option<String>, mode: ParseMode, notes: Vec<String>) -> ParsedResponse {
    ParsedResponse { changes, report: ParseReport { mode, notes, explanation } }
}

fn failed(notes: Vec<String>) -> ParsedResponse {
    ParsedResponse { changes: vec![], report: ParseReport { mode: ParseMode::Failed, notes, explanation: None } }
}

pub fn strip_think_blocks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<think>") {
        out.push_str(&rest[..start]);
        match rest[start..].find("</think>") {
            Some(end) => rest = &rest[start + end + "</think>".len()..],
            None => {
                // Unterminated reasoning: nothing after it is usable.
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

fn parse_json_object(text: &str) -> Option<Value> {
    serde_json::from_str::<Value>(text).ok().filter(|v| v.is_object() || v.is_array())
}

/// Parse allowing the most common model mistakes. Returns whether repairs were needed.
fn parse_json_lenient(text: &str) -> Option<(Value, bool)> {
    if let Some(v) = parse_json_object(text.trim()) {
        return Some((v, false));
    }
    let repaired = repair_json(text.trim());
    parse_json_object(&repaired).map(|v| (v, true))
}

/// Escape raw control characters inside strings and drop trailing commas.
pub fn repair_json(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut in_str = false;
    let mut escaped = false;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            if escaped {
                escaped = false;
                out.push(c);
            } else {
                match c {
                    '\\' => {
                        escaped = true;
                        out.push(c);
                    }
                    '"' => {
                        in_str = false;
                        out.push(c);
                    }
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                    _ => out.push(c),
                }
            }
        } else if c == '"' {
            in_str = true;
            out.push(c);
        } else if c == ',' {
            // Trailing comma before } or ].
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if !(j < chars.len() && (chars[j] == '}' || chars[j] == ']')) {
                out.push(c);
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

type Extracted = (Vec<ProposedChange>, Option<String>, Vec<String>);

fn extract_changes(v: &Value) -> Option<Extracted> {
    let mut notes = Vec::new();
    let (arr, explanation): (&Vec<Value>, Option<String>) = match v {
        Value::Object(o) => {
            let arr = o.get("changes").or_else(|| o.get("files")).and_then(Value::as_array)?;
            if !o.contains_key("changes") {
                notes.push("used `files` instead of `changes`".into());
            }
            (arr, o.get("explanation").and_then(Value::as_str).map(str::to_string))
        }
        Value::Array(a) => {
            notes.push("response was a bare array instead of an object".into());
            (a, None)
        }
        _ => return None,
    };
    let mut changes = Vec::new();
    for item in arr {
        let obj = item.as_object()?;
        let (pk, path) = PATH_KEYS.iter().find_map(|k| obj.get(*k).and_then(Value::as_str).map(|p| (*k, p)))?;
        let (ck, content) = CONTENT_KEYS.iter().find_map(|k| obj.get(*k).and_then(Value::as_str).map(|c| (*k, c)))?;
        if pk != "path" || ck != "content" {
            notes.push(format!("used non-standard keys `{pk}`/`{ck}`"));
        }
        changes.push(ProposedChange { path: path.to_string(), content: content.to_string() });
    }
    notes.dedup();
    Some((changes, explanation, notes))
}

struct Fence {
    lang: String,
    info: String,
    body: String,
    /// Text on the lines directly before the fence.
    preceding: String,
}

fn fenced_blocks(text: &str) -> Vec<Fence> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let l = lines[i].trim_start();
        let ticks = l.chars().take_while(|c| *c == '`').count();
        if ticks >= 3 {
            let info = l[ticks..].trim().to_string();
            let lang = info.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
            let mut body = Vec::new();
            let mut j = i + 1;
            let mut closed = false;
            while j < lines.len() {
                let t = lines[j].trim();
                if t.chars().all(|c| c == '`') && t.len() >= ticks {
                    closed = true;
                    break;
                }
                body.push(lines[j]);
                j += 1;
            }
            let preceding = if i > 0 { lines[i - 1].to_string() } else { String::new() };
            // An unterminated fence still carries usable text only if the model ran out of tokens; skip it.
            if closed {
                blocks.push(Fence { lang, info, body: body.join("\n"), preceding });
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    blocks
}

/// All top-level balanced `{...}` substrings (string-aware), longest first.
fn balanced_objects(text: &str) -> Vec<String> {
    let bytes: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '{' {
            let mut depth = 0i32;
            let mut in_str = false;
            let mut esc = false;
            let mut end = None;
            for (j, &c) in bytes.iter().enumerate().skip(i) {
                if in_str {
                    if esc {
                        esc = false;
                    } else if c == '\\' {
                        esc = true;
                    } else if c == '"' {
                        in_str = false;
                    }
                } else if c == '"' {
                    in_str = true;
                } else if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(j);
                        break;
                    }
                }
            }
            if let Some(e) = end {
                found.push(bytes[i..=e].iter().collect::<String>());
                i = e + 1;
                continue;
            }
        }
        i += 1;
    }
    found.sort_by_key(|s| std::cmp::Reverse(s.len()));
    found
}

fn looks_truncated(text: &str) -> bool {
    let Some(start) = text.find('{') else { return false };
    let mut depth = 0i32;
    let mut in_str = false;
    let mut esc = false;
    for c in text[start..].chars() {
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
        } else if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
        }
    }
    depth > 0 || in_str
}

fn changes_from_markdown(text: &str, editable: &[String]) -> Vec<ProposedChange> {
    let blocks = fenced_blocks(text);
    let code_blocks: Vec<&Fence> =
        blocks.iter().filter(|b| !matches!(b.lang.as_str(), "json" | "text" | "diff" | "bash" | "sh" | "shell" | "console")).collect();
    let mut out: Vec<ProposedChange> = Vec::new();
    for b in &code_blocks {
        let path = path_hint(b, editable).or_else(|| (editable.len() == 1 && code_blocks.len() == 1).then(|| editable[0].clone()));
        if let Some(p) = path {
            // Drop a leading "path:" comment line from the content.
            let content = strip_path_comment(&b.body);
            out.retain(|c| c.path != p);
            out.push(ProposedChange { path: p, content });
        }
    }
    out
}

fn path_hint(b: &Fence, editable: &[String]) -> Option<String> {
    let hay = [b.info.as_str(), b.preceding.as_str(), b.body.lines().next().unwrap_or("")];
    for h in hay {
        for e in editable {
            if h.contains(e.as_str()) {
                return Some(e.clone());
            }
        }
    }
    None
}

fn strip_path_comment(body: &str) -> String {
    let mut lines = body.lines();
    match lines.next() {
        Some(first) if first.contains("path:") || first.contains("file:") || first.contains("filename:") => {
            lines.collect::<Vec<_>>().join("\n")
        }
        _ => body.to_string(),
    }
}

/// Check proposed changes against the task's editable files.
pub struct Validated {
    pub accepted: Vec<ProposedChange>,
    pub records: Vec<AppliedChange>,
}

pub fn validate_changes(changes: Vec<ProposedChange>, editable: &[String]) -> Validated {
    let mut accepted: Vec<ProposedChange> = Vec::new();
    let mut records: Vec<AppliedChange> = Vec::new();
    for c in changes {
        let bytes = c.content.len().min(u32::MAX as usize) as u32;
        let reject = |reason: String| AppliedChange { path: c.path.clone(), bytes, status: ChangeStatus::Rejected, reason: Some(reason) };
        let norm = match normalize_rel_path(&c.path) {
            Ok(n) => n,
            Err(e) => {
                records.push(reject(e.to_string()));
                continue;
            }
        };
        if !editable.iter().any(|e| normalize_rel_path(e).ok().as_deref() == Some(norm.as_str())) {
            records.push(reject("not one of the files this task allows you to modify".into()));
            continue;
        }
        if c.content.trim().is_empty() {
            records.push(reject("empty file content".into()));
            continue;
        }
        if c.content.len() > pulsebench_sandbox::workspace::MAX_WRITE_BYTES {
            records.push(reject("file too large".into()));
            continue;
        }
        // Later changes to the same file replace earlier ones.
        if let Some(prev) = records.iter_mut().find(|r| r.path == norm && r.status == ChangeStatus::Applied) {
            prev.reason = Some("superseded by a later change to the same file".into());
            prev.status = ChangeStatus::Rejected;
            accepted.retain(|a| a.path != norm);
        }
        records.push(AppliedChange { path: norm.clone(), bytes, status: ChangeStatus::Applied, reason: None });
        accepted.push(ProposedChange { path: norm, content: c.content });
    }
    Validated { accepted, records }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ed(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|s| s.to_string()).collect()
    }

    const GOOD: &str = r#"{"changes":[{"path":"src/a.py","content":"def f():\n    return 1\n"}],"explanation":"fixed"}"#;

    #[test]
    fn clean_json_is_not_penalized() {
        let r = parse_response(GOOD, &ed(&["src/a.py"]));
        assert_eq!(r.report.mode, ParseMode::Json);
        assert_eq!(r.changes[0].path, "src/a.py");
        assert_eq!(r.changes[0].content, "def f():\n    return 1\n");
        assert_eq!(r.report.explanation.as_deref(), Some("fixed"));
        assert!(r.report.notes.is_empty());
    }

    #[test]
    fn markdown_fenced_json_is_recovered() {
        let text = format!("Here you go:\n```json\n{GOOD}\n```\nHope that helps!");
        let r = parse_response(&text, &ed(&["src/a.py"]));
        assert_eq!(r.report.mode, ParseMode::Recovered);
        assert_eq!(r.changes.len(), 1);
    }

    #[test]
    fn json_surrounded_by_prose_is_recovered() {
        let r = parse_response(&format!("Sure! {GOOD} Let me know."), &ed(&["src/a.py"]));
        assert_eq!(r.report.mode, ParseMode::Recovered);
        assert_eq!(r.changes.len(), 1);
    }

    #[test]
    fn unescaped_newlines_and_trailing_commas_are_repaired() {
        let text = "{\"changes\":[{\"path\":\"src/a.py\",\"content\":\"def f():\n    return 1\n\",}],}";
        let r = parse_response(text, &ed(&["src/a.py"]));
        assert_eq!(r.report.mode, ParseMode::Recovered, "{:?}", r.report);
        assert_eq!(r.changes[0].content, "def f():\n    return 1\n");
    }

    #[test]
    fn think_blocks_are_stripped_but_noted() {
        let r = parse_response(&format!("<think>hmm {{not json}}</think>{GOOD}"), &ed(&["src/a.py"]));
        assert_eq!(r.report.mode, ParseMode::Recovered);
        assert_eq!(r.changes.len(), 1);
    }

    #[test]
    fn unterminated_think_yields_failure() {
        let r = parse_response("<think>I should consider", &ed(&["a"]));
        assert_eq!(r.report.mode, ParseMode::Failed);
    }

    #[test]
    fn truncated_output_is_diagnosed() {
        let r = parse_response("{\"changes\":[{\"path\":\"src/a.py\",\"content\":\"def f():\\n    ret", &ed(&["src/a.py"]));
        assert_eq!(r.report.mode, ParseMode::Failed);
        assert!(r.report.notes.iter().any(|n| n.contains("truncated")), "{:?}", r.report.notes);
    }

    #[test]
    fn single_code_block_maps_to_the_only_editable_file() {
        let text = "Here is the fix:\n```python\ndef f():\n    return 2\n```\n";
        let r = parse_response(text, &ed(&["src/a.py"]));
        assert_eq!(r.report.mode, ParseMode::Recovered);
        assert_eq!(r.changes[0].path, "src/a.py");
        assert_eq!(r.changes[0].content, "def f():\n    return 2");
    }

    #[test]
    fn code_blocks_use_path_hints_with_multiple_files() {
        let text = "**src/a.py**\n```python\nA = 1\n```\n```python\n# path: src/b.py\nB = 2\n```\n";
        let r = parse_response(text, &ed(&["src/a.py", "src/b.py"]));
        assert_eq!(r.changes.len(), 2);
        assert_eq!(r.changes[1].content, "B = 2");
    }

    #[test]
    fn ambiguous_code_blocks_are_not_guessed() {
        let text = "```python\nA = 1\n```\n```python\nB = 2\n```\n";
        let r = parse_response(text, &ed(&["a.py", "b.py"]));
        assert_eq!(r.report.mode, ParseMode::Failed);
    }

    #[test]
    fn nonstandard_keys_are_accepted_with_note() {
        let r = parse_response(r#"{"files":[{"file":"src/a.py","code":"x=1"}]}"#, &ed(&["src/a.py"]));
        assert_eq!(r.report.mode, ParseMode::Recovered);
        assert_eq!(r.changes[0].content, "x=1");
    }

    #[test]
    fn garbage_and_empty_fail_cleanly() {
        assert_eq!(parse_response("", &ed(&["a"])).report.mode, ParseMode::Failed);
        assert_eq!(parse_response("I cannot help with that.", &ed(&["a"])).report.mode, ParseMode::Failed);
        assert_eq!(parse_response("{\"changes\": 5}", &ed(&["a"])).report.mode, ParseMode::Failed);
    }

    #[test]
    fn validation_rejects_disallowed_and_unsafe_paths() {
        let v = validate_changes(
            vec![
                ProposedChange { path: "src/a.py".into(), content: "ok".into() },
                ProposedChange { path: "tests/test_a.py".into(), content: "cheat".into() },
                ProposedChange { path: "../../etc/passwd".into(), content: "x".into() },
                ProposedChange { path: "/abs".into(), content: "x".into() },
                ProposedChange { path: "src/a.py".into(), content: "   ".into() },
            ],
            &ed(&["src/a.py"]),
        );
        assert_eq!(v.accepted.len(), 1);
        assert_eq!(v.records.iter().filter(|r| r.status == ChangeStatus::Rejected).count(), 4);
        assert!(v.records[1].reason.as_ref().unwrap().contains("allows you to modify"));
    }

    #[test]
    fn later_change_to_same_file_wins() {
        let v = validate_changes(
            vec![
                ProposedChange { path: "a.py".into(), content: "one".into() },
                ProposedChange { path: "./a.py".into(), content: "two".into() },
            ],
            &ed(&["a.py"]),
        );
        assert_eq!(v.accepted.len(), 1);
        assert_eq!(v.accepted[0].content, "two");
    }
}
