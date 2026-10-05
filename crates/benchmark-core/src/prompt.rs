//! Prompt construction. Identical text is produced for every model; nothing is tuned per model.

use pulsebench_types::Task;

pub const SYSTEM_PROMPT_VERSION: &str = "pulsebench-prompt-v1";

pub const SYSTEM_PROMPT: &str = "\
You are an expert software engineer completing a coding benchmark task. You are given a task \
description and the contents of the relevant files.

Respond with ONE JSON object and nothing else, in exactly this shape:

{\"changes\":[{\"path\":\"<file path>\",\"content\":\"<complete new file content>\"}],\"explanation\":\"<one or two sentences>\"}

Rules:
- \"path\" must be one of the files listed as modifiable in the task.
- \"content\" must be the COMPLETE contents of that file after your change - never a diff or a fragment.
- Escape the file content correctly so the whole response is valid JSON.
- Do not wrap the JSON in markdown fences and do not add any text outside the JSON object.
- Do not modify files that are not listed as modifiable. You cannot run commands; the benchmark runs the checks itself.";

/// A file shown to the model.
pub struct PromptFile {
    pub path: String,
    /// `None` for modifiable files that do not exist yet.
    pub content: Option<String>,
}

/// Feedback from a failed attempt, used to build the repair prompt.
pub struct RepairContext<'a> {
    pub attempt: u32,
    pub failure: &'a str,
    pub output: &'a str,
}

pub fn fence_for(content: &str) -> String {
    let longest = content.lines().map(|l| l.trim_start().chars().take_while(|c| *c == '`').count()).max().unwrap_or(0);
    "`".repeat((longest + 1).max(3))
}

fn render_file(out: &mut String, lang_tag: &str, f: &PromptFile) {
    out.push_str(&format!("### {}\n", f.path));
    match &f.content {
        Some(c) => {
            let fence = fence_for(c);
            out.push_str(&format!("{fence}{lang_tag}\n{c}"));
            if !c.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("{fence}\n\n"));
        }
        None => out.push_str("(this file does not exist yet; create it)\n\n"),
    }
}

/// Build the user prompt. `modifiable` are shown with their current contents; `context` read-only.
pub fn build_user_prompt(task: &Task, modifiable: &[PromptFile], context: &[PromptFile], repair: Option<&RepairContext<'_>>) -> String {
    let tag = task.language.fence_tag();
    let mut p = String::new();
    p.push_str(&format!("# Task: {}\n", task.title));
    p.push_str(&format!("Language: {} | Category: {:?} | Difficulty: {:?}\n\n", tag, task.category, task.difficulty));
    p.push_str(task.description.trim());
    p.push_str("\n\n## Files you may modify\n");
    for f in modifiable {
        p.push_str(&format!("- {}\n", f.path));
    }
    p.push_str("\n## Current contents of the modifiable files\n");
    for f in modifiable {
        render_file(&mut p, tag, f);
    }
    if !context.is_empty() {
        p.push_str("## Read-only context (do not modify)\n");
        for f in context {
            render_file(&mut p, tag, f);
        }
    }
    p.push_str("## How your solution is checked\n");
    if let Some(c) = &task.compile_command {
        p.push_str(&format!("1. Compile / type-check: `{c}`\n"));
        p.push_str(&format!("2. Tests: `{}`\n", task.test_command));
    } else {
        p.push_str(&format!("Tests: `{}`\n", task.test_command));
    }
    if let Some(r) = repair {
        p.push_str(&format!(
            "\n## Attempt {} failed\nFailure: {}\n\nOutput:\n```\n{}\n```\n\nThe modifiable files above already contain your previous attempt. \
             Return the complete corrected files.\n",
            r.attempt, r.failure, truncate_middle(r.output, 4000)
        ));
    }
    p.push_str("\nRespond now with the JSON object only.\n");
    p
}

/// Keep the start and end of long text.
pub fn truncate_middle(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max / 2).collect();
    let tail: String = s.chars().rev().take(max / 2).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{head}\n…[output truncated]…\n{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulsebench_types::*;

    fn task() -> Task {
        Task {
            id: "t".into(),
            title: "Fix add".into(),
            category: Category::Bugfix,
            language: Language::Python,
            difficulty: Difficulty::Easy,
            description: "Make the tests pass.".into(),
            workspace: "workspace".into(),
            entry_files: vec!["calc.py".into()],
            editable_files: None,
            context_files: vec![],
            compile_command: None,
            test_command: "python -m unittest".into(),
            timeout_seconds: 60,
            max_attempts: 1,
            verification: Verification::Tests,
        }
    }

    #[test]
    fn prompt_is_deterministic_and_contains_files() {
        let t = task();
        let files = [PromptFile { path: "calc.py".into(), content: Some("def add(a,b): return a-b\n".into()) }];
        let ctx = [PromptFile { path: "test_calc.py".into(), content: Some("import unittest\n".into()) }];
        let a = build_user_prompt(&t, &files, &ctx, None);
        let b = build_user_prompt(&t, &files, &ctx, None);
        assert_eq!(a, b);
        assert!(a.contains("### calc.py"));
        assert!(a.contains("def add(a,b): return a-b"));
        assert!(a.contains("Read-only context"));
        assert!(a.contains("`python -m unittest`"));
        assert!(!a.contains("Attempt 1 failed"));
    }

    #[test]
    fn fences_grow_around_embedded_backticks() {
        assert_eq!(fence_for("no ticks"), "```");
        assert_eq!(fence_for("x\n```js\ny\n```\n"), "````");
    }

    #[test]
    fn missing_modifiable_files_are_announced() {
        let t = task();
        let files = [PromptFile { path: "tests/test_new.py".into(), content: None }];
        assert!(build_user_prompt(&t, &files, &[], None).contains("does not exist yet"));
    }

    #[test]
    fn repair_prompt_includes_truncated_output() {
        let t = task();
        let files = [PromptFile { path: "calc.py".into(), content: Some("x".into()) }];
        let long = "e".repeat(10_000);
        let r = RepairContext { attempt: 1, failure: "tests failed", output: &long };
        let p = build_user_prompt(&t, &files, &[], Some(&r));
        assert!(p.contains("Attempt 1 failed"));
        assert!(p.contains("output truncated"));
        assert!(p.len() < 8_000);
    }
}
