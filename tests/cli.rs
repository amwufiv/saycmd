use std::process::Command;

#[test]
fn init_zsh_prints_loadable_integration() {
    let binary = env!("CARGO_BIN_EXE_saycmd");
    let temp = tempfile::tempdir().unwrap();
    let output = Command::new(binary)
        .args(["init", "zsh"])
        .env("XDG_CONFIG_HOME", temp.path())
        .env_remove("SAYCMD_PREFIX")
        .output()
        .unwrap();
    assert!(output.status.success());
    let script = String::from_utf8(output.stdout).unwrap();
    assert!(script.contains("zle -N accept-line"));

    let syntax = Command::new("zsh")
        .args(["-n", "-c", &script])
        .output()
        .unwrap();
    assert!(
        syntax.status.success(),
        "{}",
        String::from_utf8_lossy(&syntax.stderr)
    );
}

#[test]
fn missing_model_is_a_clear_error() {
    let binary = env!("CARGO_BIN_EXE_saycmd");
    let temp = tempfile::tempdir().unwrap();
    let output = Command::new(binary)
        .args(["translate", "--", "hello"])
        .env("XDG_CONFIG_HOME", temp.path())
        .env_remove("SAYCMD_MODEL")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("model is not configured"));
}

#[test]
fn init_uses_configured_prefix_and_environment_override_as_literal_text() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("saycmd");
    std::fs::create_dir(&directory).unwrap();
    for (file, override_value, expected) in [
        ("", None, "？"),
        ("prefix = '??'", None, "??"),
        ("prefix = ''", None, "？"),
        ("prefix = '   '", None, "？"),
        ("prefix = '?'", Some("！"), "！"),
        ("prefix = '?'", Some(""), "?"),
        (
            "prefix = \"'$(echo injected)`echo injected`*$HOME\"",
            None,
            "'$(echo injected)`echo injected`*$HOME",
        ),
    ] {
        std::fs::write(directory.join("config.toml"), file).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_saycmd"));
        command
            .args(["init", "zsh"])
            .env("XDG_CONFIG_HOME", temp.path())
            .env_remove("SAYCMD_MODEL")
            .env_remove("SAYCMD_PREFIX");
        if let Some(value) = override_value {
            command.env("SAYCMD_PREFIX", value);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let script = String::from_utf8(output.stdout).unwrap();
        // Stub ZLE and submit only the prefix: recognition must report an empty prompt.
        let script = format!("zle() {{ if [[ $1 == -M ]]; then print -r -- \"$2\"; fi; }}\n{script}\nBUFFER=$TEST_PREFIX\n_saycmd_accept_line");
        let output = Command::new("zsh")
            .args(["-f", "-c", &script])
            .env_remove("SAYCMD_PREFIX")
            .env("TEST_PREFIX", expected)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "saycmd: prompt is empty\n"
        );
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn shell_handles_translation_success_and_failure_without_executing_output() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
    let mock = temp.path().join("saycmd");
    std::fs::write(
        &mock,
        r#"#!/bin/sh
sleep 0.25
printf '%s' "$MOCK_OUTPUT"
if [ "$MOCK_EXIT" = 0 ]; then printf '\000%s' "$MOCK_EXPLANATION"; fi
exit "$MOCK_EXIT"
"#,
    )
    .unwrap();
    std::fs::set_permissions(&mock, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(temp.path().to_path_buf())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let integration = saycmd::shell::zsh_init("?");
    let script = format!(
        r#"
typeset -a progress=()
zle() {{
  if [[ $1 == -M ]]; then
    if [[ $2 == -- ]]; then last_message=$3; else last_message=$2; fi
  elif [[ $1 == -R ]]; then
    progress+=("$2")
  fi
}}
{integration}
BUFFER='?列出当前目录文件'
_saycmd_accept_line
result=$?
[[ ${{#progress}} -ge 2 && "${{progress[1]}}" != "${{progress[2]}}" ]] || exit 90
[[ "${{progress[1]}}" == 'saycmd: '*generating* ]] || exit 91
print -r -- "$result"
print -r -- "$BUFFER"
print -r -- "$CURSOR"
print -r -- "$last_message"
"#
    );
    for (exit_code, generated, expected_buffer, expected_cursor, explanation, expected_message) in [
        (
            "0",
            "echo SHOULD_NOT_EXECUTE",
            "echo SHOULD_NOT_EXECUTE",
            "23",
            "",
            "",
        ),
        (
            "0",
            "ls -la",
            "ls -la",
            "6",
            "-l：详细列表\n-a：包含隐藏文件",
            "-l：详细列表\n-a：包含隐藏文件",
        ),
        (
            "7",
            "saycmd: provider unavailable",
            "?列出当前目录文件",
            "",
            "",
            "saycmd: provider unavailable",
        ),
    ] {
        let output = Command::new("zsh")
            .args(["-f", "-c", &script])
            .env("PATH", &path)
            .env_remove("SAYCMD_PREFIX")
            .env("MOCK_OUTPUT", generated)
            .env("MOCK_EXIT", exit_code)
            .env("MOCK_EXPLANATION", explanation)
            .env("TMPDIR", temp.path())
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("{exit_code}\n{expected_buffer}\n{expected_cursor}\n{expected_message}\n")
        );
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
    }
}

#[test]
fn followups_preserve_context_on_failure_and_reset_on_new_generation() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let mock = temp.path().join("saycmd");
    let log = temp.path().join("calls.jsonl");
    std::fs::write(
        &mock,
        r#"#!/usr/bin/python3
import json, os, sys
with open(os.environ['CALL_LOG'], 'a') as f:
    f.write(json.dumps(sys.argv[1:]) + '\n')
if os.environ.get('MOCK_FAIL') == '1':
    print('saycmd: unavailable', file=sys.stderr)
    sys.exit(7)
sys.stdout.write(os.environ['MOCK_COMMAND'] + '\0')
"#,
    )
    .unwrap();
    std::fs::set_permissions(&mock, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(temp.path().to_path_buf())
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let script = format!(
        r#"
set -e
zle() {{ :; }}
{}
BUFFER='??sort by time'
if _saycmd_accept_line; then exit 11; fi
[[ "$BUFFER" == '??sort by time' ]]
export MOCK_COMMAND='ls'
BUFFER='?list files'
_saycmd_accept_line
[[ "$BUFFER" == ls ]]
# Ordinary execution does not erase the follow-up context.
_saycmd_accept_line
export MOCK_COMMAND='ls -t'
BUFFER='??sort by time'
_saycmd_accept_line
[[ "$BUFFER" == 'ls -t' ]]
export MOCK_FAIL=1
BUFFER='??include hidden'
if _saycmd_accept_line; then exit 12; fi
[[ "$BUFFER" == '??include hidden' && "$_saycmd_last_command" == 'ls -t' ]]
export MOCK_FAIL=0 MOCK_COMMAND='ls -ta'
_saycmd_accept_line
[[ "$BUFFER" == 'ls -ta' ]]
export MOCK_COMMAND='pwd'
BUFFER='?show directory'
_saycmd_accept_line
BUFFER='??physical path'
_saycmd_accept_line
# Reloading integration keeps this session's context.
{}
[[ "$_saycmd_last_command" == pwd && "${{#_saycmd_requests}}" == 2 ]]
for n in {{1..10}}; do BUFFER="??revision $n"; _saycmd_accept_line; done
[[ "${{#_saycmd_requests}}" == 8 && "${{_saycmd_requests[1]}}" == 'show directory' && "${{_saycmd_requests[2]}}" == 'revision 4' ]]
"#,
        saycmd::shell::zsh_init("?"),
        saycmd::shell::zsh_init("?")
    );
    let output = Command::new("zsh")
        .args(["-f", "-c", &script])
        .env("PATH", path)
        .env("CALL_LOG", &log)
        .env_remove("SAYCMD_PREFIX")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let calls: Vec<Vec<String>> = std::fs::read_to_string(log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(!calls[0].iter().any(|arg| arg.starts_with("--previous-")));
    assert!(calls[1].contains(&"--previous-command=ls".into()));
    assert!(calls[1].contains(&"--previous-request=list files".into()));
    assert_eq!(calls[1].last().unwrap(), "sort by time");
    assert_eq!(calls[2], calls[3]); // Failed attempts are not added to history.
    assert!(calls[3].contains(&"--previous-command=ls -t".into()));
    assert!(calls[3].contains(&"--previous-request=sort by time".into()));
    assert!(!calls[4].iter().any(|arg| arg.starts_with("--previous-")));
    assert!(calls[5].contains(&"--previous-request=show directory".into()));
    assert!(!calls[5].contains(&"--previous-request=list files".into()));
}
