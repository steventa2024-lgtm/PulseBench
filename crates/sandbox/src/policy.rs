//! Command allowlist and environment sanitization.
//!
//! Commands always come from trusted benchmark definitions; this policy is a second line of
//! defence so a malformed or malicious suite cannot run arbitrary programs, and so nothing from
//! the user's environment leaks into benchmark processes.

use std::path::Path;

use crate::error::{Result, SandboxError};

/// Programs benchmark definitions may invoke (logical names, no extension).
pub const ALLOWED_PROGRAMS: &[&str] = &["python", "python3", "py", "node", "npm"];

/// Flags that execute inline code supplied on the command line.
const INLINE_CODE_FLAGS: &[(&str, &[&str])] =
    &[("python", &["-c"]), ("python3", &["-c"]), ("py", &["-c"]), ("node", &["-e", "--eval", "-p", "--print"])];

/// Interpreter options whose next argument is a value, not the script.
const VALUE_OPTIONS: &[&str] = &["-r", "--require", "--import", "--loader", "--experimental-loader", "-W", "-X"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCommand {
    /// Logical program name (`python`, `node`, `npm`).
    pub program: String,
    pub args: Vec<String>,
}

/// Split a command line (no shell involved) and validate it against the allowlist.
pub fn parse_and_validate(command: &str) -> Result<ParsedCommand> {
    let mut parts = shell_words::split(command).map_err(|e| SandboxError::CommandRejected(format!("cannot parse `{command}`: {e}")))?;
    if parts.is_empty() {
        return Err(SandboxError::CommandRejected("empty command".into()));
    }
    let first = parts.remove(0);
    let name = Path::new(&first).file_stem().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
    if first.contains('/') || first.contains('\\') {
        return Err(SandboxError::CommandRejected(format!("`{first}`: programs must be named, not given as a path")));
    }
    if !ALLOWED_PROGRAMS.contains(&name.as_str()) {
        return Err(SandboxError::CommandRejected(format!(
            "`{first}` is not an allowed program (allowed: {})",
            ALLOWED_PROGRAMS.join(", ")
        )));
    }
    if let Some((_, flags)) = INLINE_CODE_FLAGS.iter().find(|(p, _)| *p == name) {
        // Only interpreter options (before the script/module name) are inspected; arguments after
        // the script belong to the script (`tsc -p .`).
        let mut prev_takes_value = false;
        for a in &parts {
            if flags.contains(&a.as_str()) {
                return Err(SandboxError::CommandRejected(format!("`{a}` (inline code) is not allowed")));
            }
            if !a.starts_with('-') && !prev_takes_value {
                break;
            }
            prev_takes_value = VALUE_OPTIONS.contains(&a.as_str());
        }
    }
    if parts.iter().any(|a| a.contains('\0')) {
        return Err(SandboxError::CommandRejected("NUL byte in arguments".into()));
    }
    Ok(ParsedCommand { program: name, args: parts })
}

/// Build the complete environment for a benchmark process.
///
/// `host_path` is the user's PATH (needed to find interpreters; it is not a secret).
pub fn sanitized_env(workspace: &Path, host_path: &str, node_bin: Option<&Path>) -> Vec<(String, String)> {
    let home = workspace.join(".pb-home");
    let tmp = workspace.join(".pb-tmp");
    let s = |p: &Path| p.to_string_lossy().into_owned();

    let mut path = String::new();
    if let Some(nb) = node_bin {
        path.push_str(&s(nb));
        path.push(if cfg!(windows) { ';' } else { ':' });
    }
    path.push_str(host_path);

    let mut env: Vec<(String, String)> = vec![
        ("PATH".into(), path),
        ("HOME".into(), s(&home)),
        ("USERPROFILE".into(), s(&home)),
        ("APPDATA".into(), s(&home.join("AppData"))),
        ("LOCALAPPDATA".into(), s(&home.join("AppData"))),
        ("XDG_CACHE_HOME".into(), s(&home.join(".cache"))),
        ("XDG_CONFIG_HOME".into(), s(&home.join(".config"))),
        ("TMPDIR".into(), s(&tmp)),
        ("TEMP".into(), s(&tmp)),
        ("TMP".into(), s(&tmp)),
        ("CI".into(), "1".into()),
        ("NO_COLOR".into(), "1".into()),
        ("FORCE_COLOR".into(), "0".into()),
        ("NODE_ENV".into(), "test".into()),
        ("PYTHONDONTWRITEBYTECODE".into(), "1".into()),
        ("PYTHONHASHSEED".into(), "0".into()),
        ("PYTHONUTF8".into(), "1".into()),
        ("PYTHONIOENCODING".into(), "utf-8".into()),
        ("npm_config_cache".into(), s(&home.join(".npm"))),
        ("npm_config_update_notifier".into(), "false".into()),
        ("npm_config_fund".into(), "false".into()),
        ("npm_config_audit".into(), "false".into()),
        ("LANG".into(), "C.UTF-8".into()),
    ];
    if cfg!(windows) {
        // Windows processes cannot start without these.
        for key in ["SystemRoot", "SYSTEMDRIVE", "WINDIR", "COMSPEC", "PATHEXT", "NUMBER_OF_PROCESSORS", "PROCESSOR_ARCHITECTURE"] {
            if let Ok(v) = std::env::var(key) {
                env.push((key.into(), v));
            }
        }
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_trusted_commands() {
        let c = parse_and_validate("python -m unittest discover -s tests").unwrap();
        assert_eq!(c.program, "python");
        assert_eq!(c.args, vec!["-m", "unittest", "discover", "-s", "tests"]);
        let c = parse_and_validate("node node_modules/typescript/bin/tsc -p .").unwrap();
        assert_eq!(c.program, "node");
        assert!(parse_and_validate("npm test").is_ok());
    }

    #[test]
    fn rejects_other_programs_and_paths() {
        for bad in ["rm -rf /", "curl http://x", "bash -c ls", "/usr/bin/python test.py", "sh run.sh", "powershell x", ""] {
            assert!(parse_and_validate(bad).is_err(), "should reject `{bad}`");
        }
    }

    #[test]
    fn rejects_inline_code_flags() {
        assert!(parse_and_validate("python -c 'import os'").is_err());
        assert!(parse_and_validate("node -e 'process.exit(0)'").is_err());
        assert!(parse_and_validate("node --eval x").is_err());
        assert!(parse_and_validate("node --test").is_ok());
        assert!(parse_and_validate("node --require ./x.js -e 1").is_err());
        assert!(parse_and_validate("node node_modules/typescript/bin/tsc -p .").is_ok(), "-p after the script belongs to the script");
    }

    #[test]
    fn env_contains_no_inherited_secrets() {
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "x");
        std::env::set_var("GITHUB_TOKEN", "x");
        let env = sanitized_env(Path::new("/tmp/ws"), "/usr/bin", None);
        let keys: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
        assert!(!keys.contains(&"AWS_SECRET_ACCESS_KEY"));
        assert!(!keys.contains(&"GITHUB_TOKEN"));
        let home = env.iter().find(|(k, _)| k == "HOME").unwrap();
        let expected = Path::new("/tmp/ws").join(".pb-home").to_string_lossy().into_owned(); // separator differs per OS
        assert_eq!(home.1, expected, "HOME must point inside the workspace");
    }
}
