use similar::TextDiff;

/// Unified diff between two versions of a file (`None` = file did not exist).
pub fn unified_diff(path: &str, old: Option<&str>, new: &str) -> String {
    let old_text = old.unwrap_or("");
    let a = if old.is_some() { format!("a/{path}") } else { "/dev/null".to_string() };
    let b = format!("b/{path}");
    TextDiff::from_lines(old_text, new).unified_diff().context_radius(3).header(&a, &b).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_shows_changed_lines() {
        let d = unified_diff("a.py", Some("x = 1\ny = 2\n"), "x = 1\ny = 3\n");
        assert!(d.contains("--- a/a.py"));
        assert!(d.contains("-y = 2"));
        assert!(d.contains("+y = 3"));
    }

    #[test]
    fn new_files_diff_against_dev_null() {
        let d = unified_diff("t.py", None, "print(1)\n");
        assert!(d.contains("--- /dev/null"));
        assert!(d.contains("+print(1)"));
    }

    #[test]
    fn identical_files_have_empty_diff() {
        assert_eq!(unified_diff("a", Some("same\n"), "same\n"), "");
    }
}
