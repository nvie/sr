//! End-to-end tests: these drive the actual `sr` binary, covering argument parsing, the
//! stdout/stderr split and the process exit status.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::{Builder, TempDir};

fn tempdir() -> TempDir {
    // Not `TempDir::new()`: that yields a `.tmpXXXX` directory, which `sr` skips as hidden
    Builder::new().prefix("sr-test").tempdir().unwrap()
}

fn sr(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sr"))
        .args(args)
        .output()
        .expect("failed to run sr")
}

fn sr_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sr"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to run sr")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

/// Drops ANSI colour codes, so assertions can match the text `sr` highlights
fn strip_ansi(s: &str) -> String {
    let mut rv = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for escaped in chars.by_ref() {
                if escaped == 'm' {
                    break;
                }
            }
        } else {
            rv.push(c);
        }
    }
    rv
}

fn write_file(dir: &Path, name: &str, content: &str) -> String {
    let path = dir.join(name);
    fs::write(&path, content).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn previews_matches() {
    let tmp = tempdir();
    let path = write_file(tmp.path(), "a.txt", "first\nneedle here\n");

    let output = sr(&["-s", "needle", &path]);

    assert!(output.status.success());
    assert!(strip_ansi(&stdout(&output)).contains(&path));
    assert!(strip_ansi(&stdout(&output)).contains("    2   needle here"));
    assert_eq!(stderr(&output), "");
    // Preview mode leaves the file alone
    assert_eq!(fs::read_to_string(&path).unwrap(), "first\nneedle here\n");
}

#[test]
fn replaces_matches_in_place() {
    let tmp = tempdir();
    let path = write_file(tmp.path(), "a.txt", "one two one\n");

    let output = sr(&["-s", "one", "-r", "three", &path]);

    assert!(output.status.success());
    assert!(stdout(&output).ends_with("1 files changed\n"));
    assert_eq!(fs::read_to_string(&path).unwrap(), "three two three\n");
}

#[test]
fn reports_binary_files_when_verbose() {
    let tmp = tempdir();
    let path = write_file(tmp.path(), "a.bin", "needle\0\0\0");

    let quiet = sr(&["-s", "needle", &path]);
    assert_eq!(stdout(&quiet), "");
    assert_eq!(stderr(&quiet), "");

    let verbose = sr(&["-s", "needle", "-v", &path]);
    assert_eq!(stdout(&verbose), "");
    assert!(stderr(&verbose).contains("(binary)"));
}

#[test]
fn fails_on_an_invalid_regex() {
    let output = sr(&["-s", "foo("]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("Not a valid regex: `foo(`"));
}

#[test]
fn fails_without_a_search_pattern() {
    let output = sr(&[]);

    assert!(!output.status.success());
    assert!(stderr(&output).contains("--search"));
}

#[test]
fn prints_its_version() {
    let output = sr(&["--version"]);

    assert!(output.status.success());
    assert!(stdout(&output).contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn refuses_to_run_outside_a_git_repository() {
    let tmp = tempdir();
    let path = write_file(tmp.path(), "a.txt", "needle\n");

    // Without paths, and with an explicit path: neither is allowed outside a repository
    for args in [vec!["-s", "needle"], vec!["-s", "needle", &path]] {
        let output = sr_in(tmp.path(), &args);

        assert!(!output.status.success());
        assert!(stderr(&output).contains("Not in a Git repository"));
    }

    // The file it was pointed at is untouched
    assert_eq!(fs::read_to_string(&path).unwrap(), "needle\n");
}
