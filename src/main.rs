use ansi_term::Colour::{Blue, Fixed, Purple};
use anyhow::{Context, Result};
use clap::Parser;
use content_inspector::inspect;
use ctrlc::set_handler;
use regex::{Regex, RegexBuilder};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::from_utf8;
use std::string::String;
use walkdir::{DirEntry, WalkDir};

#[derive(Parser)]
#[clap(version, about, long_about = None)]
struct CliArgs {
    #[clap(short, long)]
    verbose: bool,

    #[clap(short = 'i', long = "case-insensitive")]
    case_insensitive: bool,

    #[clap(short = 's', long = "search")]
    pattern: String,

    #[clap(short = 'r', long = "replace")]
    replace: Option<String>,

    #[clap(parse(from_os_str))]
    paths: Vec<PathBuf>,
}

fn setup_ctrlc() {
    // Set automatic Ctrl+C handling
    set_handler(move || {
        println!("received Ctrl+C!");
    })
    .expect("Error setting Ctrl-C handler");
}

fn files_from_git() -> Result<Vec<PathBuf>> {
    // Lists all local files under Git control
    let out = Command::new("git")
        .arg("ls-files")
        .args([
            "--cached",
            "--modified",
            "--exclude-standard",
            "--deduplicate",
        ])
        .output()?;

    // TODO: It's a known thing that git ls-files will "escape" file names with weird characters in
    // them. We should read up on how to convert those back to normal strings.

    let raw = from_utf8(&out.stdout)?;
    Ok(raw.lines().map(PathBuf::from).collect())
}

fn basename(path: &Path) -> Option<&str> {
    path.file_name().and_then(|basename| basename.to_str())
}

fn is_hidden(entry: &DirEntry) -> bool {
    basename(entry.path()).map_or(false, |s| s.starts_with('.'))
}

fn is_file(entry: &DirEntry) -> bool {
    entry.metadata().unwrap().is_file()
}

fn is_binary_file(path: &Path) -> Result<bool> {
    let mut buf = [0; 1024];
    let mut f = fs::File::open(path)?;
    let size = f.read(&mut buf)?;
    Ok(inspect(&buf[..size]).is_binary())
}

fn is_text_file(path: &Path) -> bool {
    !(is_binary_file(path).unwrap_or(true))
}

fn include_entry(entry: &DirEntry) -> bool {
    is_file(entry) || !is_hidden(entry)
}

fn build_regex(pattern: &str, case_insensitive: bool) -> Result<Regex> {
    RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .build()
        .with_context(|| format!("Not a valid regex: `{}`", pattern))
}

fn walk_files(roots: &[PathBuf]) -> impl Iterator<Item = PathBuf> + '_ {
    // Expands the given roots into every (non-hidden-directory) file below them, lazily: files are
    // yielded (and processed) while the tree is still being walked
    roots
        .iter()
        .flat_map(|root| {
            WalkDir::new(root)
                .into_iter()
                .filter_entry(include_entry)
                .filter_map(|entry| entry.ok())
        })
        .filter(is_file)
        .map(|entry| entry.into_path())
}

fn highlight_matches(re: &Regex, line: &str) -> Option<String> {
    let mut rv = String::from(line);

    for m in Vec::from_iter(re.find_iter(line)).into_iter().rev() {
        rv = format!(
            "{}{}{}",
            &rv[..m.start()],
            Purple.paint(&rv[m.start()..m.end()]),
            &rv[m.end()..]
        )
    }

    if rv == line {
        None
    } else {
        Some(rv)
    }
}

fn print_matches(
    re: &Regex,
    pathstr: &str,
    content: &str,
    last: &mut String,
    out: &mut impl Write,
) -> Result<()> {
    for (lineno0, line) in content.lines().enumerate() {
        let lineno = lineno0 + 1;

        let highlighted: Option<String> = highlight_matches(re, line);
        if let Some(hline) = highlighted {
            if last != pathstr {
                if !last.is_empty() {
                    // Print separator unless this is the first loop
                    writeln!(out)?;
                }
                writeln!(out, "{}", Blue.paint(pathstr))?;
                *last = String::from(pathstr);
            }

            writeln!(
                out,
                "{}   {}",
                Fixed(248).dimmed().paint(format!("{:5}", lineno)),
                hline
            )?;
        }
    }
    Ok(())
}

fn replace_in_file(re: &Regex, path: &Path, content: &str, replacement: &str) -> Result<bool> {
    let new_content = re.replace_all(content, replacement);
    if content == new_content {
        return Ok(false);
    }

    fs::write(path, &*new_content)
        .with_context(|| format!("Unable to write to file `{}`", path.display()))?;
    Ok(true)
}

fn run(args: &CliArgs, out: &mut impl Write, err: &mut impl Write) -> Result<()> {
    let re = build_regex(&args.pattern, args.case_insensitive)?;

    let mut last: String = String::new();

    let all_paths: Vec<PathBuf> = if !args.paths.is_empty() {
        args.paths.clone()
    } else {
        files_from_git().with_context(|| "No input files specified".to_string())?
    };

    let mut count = 0;

    for path in walk_files(&all_paths) {
        let pathstr: &str = path
            .to_str()
            .with_context(|| "Error in file name".to_string())?;

        if !is_text_file(&path) {
            if args.verbose {
                writeln!(
                    err,
                    "{} (binary)",
                    Fixed(248).dimmed().paint(pathstr.to_string())
                )?;
            }
            continue;
        }

        let content = match fs::read_to_string(&path) {
            Err(_) => {
                writeln!(err, "{} (read error)", pathstr)?;
                continue;
            }
            Ok(content) => content,
        };

        if let Some(ref replacement) = args.replace {
            // Inline-replace matches
            if replace_in_file(&re, &path, &content, replacement)? {
                count += 1;
                writeln!(out, "{}", path.display())?;
            } else {
                writeln!(
                    out,
                    "{}",
                    Fixed(248).dimmed().paint(format!("{}", path.display()))
                )?;
            }
        } else {
            // Highlight matches
            print_matches(&re, pathstr, &content, &mut last, out)?;
        }
    }

    if args.replace.is_some() {
        if count > 0 {
            writeln!(out, "{} files changed", count)?;
        } else {
            writeln!(out, "No files changed")?;
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    setup_ctrlc();

    let args = CliArgs::parse();
    run(&args, &mut io::stdout(), &mut io::stderr())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use tempfile::{Builder, TempDir};

    fn tempdir() -> TempDir {
        // Not `TempDir::new()`: that yields a `.tmpXXXX` directory, which `sr` skips as hidden
        Builder::new().prefix("sr-test").tempdir().unwrap()
    }

    fn strip_ansi(s: &str) -> String {
        Regex::new("\u{1b}\\[[0-9;]*m")
            .unwrap()
            .replace_all(s, "")
            .into_owned()
    }

    fn write_file(dir: &Path, name: &str, content: impl AsRef<[u8]>) -> PathBuf {
        let path = dir.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

    fn entry_named(root: &Path, name: &str) -> DirEntry {
        WalkDir::new(root)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .find(|entry| basename(entry.path()) == Some(name))
            .unwrap_or_else(|| panic!("no entry named `{}` below {}", name, root.display()))
    }

    fn args(pattern: &str, paths: &[&Path]) -> CliArgs {
        CliArgs {
            verbose: false,
            case_insensitive: false,
            pattern: String::from(pattern),
            replace: None,
            paths: paths.iter().map(PathBuf::from).collect(),
        }
    }

    /// Runs `run()` against captured buffers, returning (stdout, stderr) with ANSI codes stripped.
    fn capture(args: &CliArgs) -> (String, String) {
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        run(args, &mut out, &mut err).unwrap();
        (
            strip_ansi(from_utf8(&out).unwrap()),
            strip_ansi(from_utf8(&err).unwrap()),
        )
    }

    #[test]
    fn basename_of_paths() {
        assert_eq!(basename(Path::new("foo/bar/baz.txt")), Some("baz.txt"));
        assert_eq!(basename(Path::new("baz.txt")), Some("baz.txt"));
        assert_eq!(basename(Path::new("foo/..")), None);
        assert_eq!(basename(Path::new("/")), None);
    }

    #[cfg(unix)]
    #[test]
    fn basename_of_non_utf8_path() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let path = PathBuf::from(OsStr::from_bytes(b"/tmp/inv\xffalid"));
        assert_eq!(basename(&path), None);
    }

    #[test]
    fn hidden_files_and_dirs() {
        let tmp = tempdir();
        write_file(tmp.path(), "visible.txt", "hi");
        write_file(tmp.path(), ".hidden.txt", "hi");
        fs::create_dir(tmp.path().join(".git")).unwrap();

        assert!(!is_hidden(&entry_named(tmp.path(), "visible.txt")));
        assert!(is_hidden(&entry_named(tmp.path(), ".hidden.txt")));
        assert!(is_hidden(&entry_named(tmp.path(), ".git")));
    }

    #[test]
    fn files_versus_dirs() {
        let tmp = tempdir();
        write_file(tmp.path(), "sub/file.txt", "hi");

        assert!(is_file(&entry_named(tmp.path(), "file.txt")));
        assert!(!is_file(&entry_named(tmp.path(), "sub")));
    }

    #[test]
    fn hidden_files_are_included_but_hidden_dirs_are_not() {
        let tmp = tempdir();
        write_file(tmp.path(), ".hidden.txt", "hi");
        write_file(tmp.path(), "visible.txt", "hi");
        write_file(tmp.path(), ".git/config", "hi");
        write_file(tmp.path(), "sub/nested.txt", "hi");

        assert!(include_entry(&entry_named(tmp.path(), ".hidden.txt")));
        assert!(include_entry(&entry_named(tmp.path(), "visible.txt")));
        assert!(include_entry(&entry_named(tmp.path(), "sub")));
        assert!(!include_entry(&entry_named(tmp.path(), ".git")));
    }

    #[test]
    fn binary_detection() {
        let tmp = tempdir();
        let text = write_file(tmp.path(), "text.txt", "just some text\n");
        let binary = write_file(tmp.path(), "binary.bin", b"abc\0\0\0def");
        let empty = write_file(tmp.path(), "empty.txt", "");

        assert!(!is_binary_file(&text).unwrap());
        assert!(is_binary_file(&binary).unwrap());
        assert!(!is_binary_file(&empty).unwrap());
        assert!(is_binary_file(&tmp.path().join("nope.txt")).is_err());

        assert!(is_text_file(&text));
        assert!(!is_text_file(&binary));
        // Unreadable files are treated as binary, i.e. not text
        assert!(!is_text_file(&tmp.path().join("nope.txt")));
    }

    #[test]
    fn regexes_are_built_with_case_sensitivity() {
        assert!(build_regex("foo", false).unwrap().is_match("foo"));
        assert!(!build_regex("foo", false).unwrap().is_match("FOO"));
        assert!(build_regex("foo", true).unwrap().is_match("FOO"));
    }

    #[test]
    fn invalid_regexes_are_reported() {
        let err = build_regex("foo(", false).unwrap_err();
        assert_eq!(format!("{}", err), "Not a valid regex: `foo(`");
    }

    #[test]
    fn walking_roots_yields_files_only() {
        let tmp = tempdir();
        write_file(tmp.path(), "a.txt", "hi");
        write_file(tmp.path(), ".hidden.txt", "hi");
        write_file(tmp.path(), "sub/b.txt", "hi");
        write_file(tmp.path(), ".git/config", "hi");

        let found: HashSet<PathBuf> = walk_files(&[tmp.path().to_path_buf()])
            .map(|path| path.strip_prefix(tmp.path()).unwrap().to_path_buf())
            .collect();

        assert_eq!(
            found,
            HashSet::from([
                PathBuf::from("a.txt"),
                PathBuf::from(".hidden.txt"),
                PathBuf::from("sub/b.txt"),
            ])
        );
    }

    #[test]
    fn walking_accepts_multiple_roots_and_plain_files() {
        let tmp = tempdir();
        let a = write_file(tmp.path(), "one/a.txt", "hi");
        let b = write_file(tmp.path(), "two/b.txt", "hi");

        assert_eq!(
            walk_files(&[a.clone(), b.clone()]).collect::<Vec<_>>(),
            vec![a, b]
        );
    }

    #[test]
    fn walking_skips_hidden_roots() {
        let tmp = tempdir();
        let hidden = tmp.path().join(".hidden");
        write_file(&hidden, "a.txt", "hi");

        assert!(walk_files(&[hidden]).next().is_none());
    }

    #[test]
    fn walking_ignores_nonexistent_roots() {
        assert!(walk_files(&[PathBuf::from("/no/such/path/at/all")])
            .next()
            .is_none());
    }

    #[test]
    fn highlighting_a_line_without_matches() {
        let re = build_regex("nope", false).unwrap();
        assert_eq!(highlight_matches(&re, "hello world"), None);
    }

    #[test]
    fn highlighting_wraps_every_match() {
        let re = build_regex("o", false).unwrap();
        let hline = highlight_matches(&re, "hello world").unwrap();

        assert_ne!(hline, "hello world");
        assert_eq!(strip_ansi(&hline), "hello world");
        assert_eq!(hline.matches(&format!("{}", Purple.prefix())).count(), 2);
    }

    #[test]
    fn highlighting_uses_the_regex_flags() {
        let re = build_regex("hello", true).unwrap();
        let hline = highlight_matches(&re, "HELLO there").unwrap();
        assert_eq!(strip_ansi(&hline), "HELLO there");
    }

    #[test]
    fn printing_matches_writes_a_header_once_per_file() {
        let re = build_regex("match", false).unwrap();
        let mut last = String::new();
        let mut out: Vec<u8> = Vec::new();

        print_matches(
            &re,
            "a.txt",
            "no\nmatch here\nmatch again\n",
            &mut last,
            &mut out,
        )
        .unwrap();
        print_matches(&re, "b.txt", "nothing\n", &mut last, &mut out).unwrap();
        print_matches(&re, "c.txt", "a match\n", &mut last, &mut out).unwrap();

        assert_eq!(
            strip_ansi(from_utf8(&out).unwrap()),
            "a.txt\n    2   match here\n    3   match again\n\nc.txt\n    1   a match\n"
        );
        assert_eq!(last, "c.txt");
    }

    #[test]
    fn replacing_in_a_file() {
        let tmp = tempdir();
        let path = write_file(tmp.path(), "a.txt", "one two one\n");
        let re = build_regex("one", false).unwrap();

        assert!(replace_in_file(&re, &path, "one two one\n", "three").unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "three two three\n");
    }

    #[test]
    fn replacing_supports_capture_groups() {
        let tmp = tempdir();
        let path = write_file(tmp.path(), "a.txt", "hello world\n");
        let re = build_regex("(hello) (world)", false).unwrap();

        assert!(replace_in_file(&re, &path, "hello world\n", "$2 $1").unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "world hello\n");
    }

    #[test]
    fn replacing_without_matches_leaves_the_file_alone() {
        let tmp = tempdir();
        let path = write_file(tmp.path(), "a.txt", "untouched\n");
        let re = build_regex("nope", false).unwrap();

        assert!(!replace_in_file(&re, &path, "untouched\n", "yes").unwrap());
        assert_eq!(fs::read_to_string(&path).unwrap(), "untouched\n");
    }

    #[test]
    fn replacing_reports_unwritable_files() {
        let tmp = tempdir();
        let path = tmp.path().join("no-such-dir/a.txt");
        let re = build_regex("a", false).unwrap();

        let err = replace_in_file(&re, &path, "a\n", "b").unwrap_err();
        assert_eq!(
            format!("{}", err),
            format!("Unable to write to file `{}`", path.display())
        );
    }

    #[test]
    fn running_in_preview_mode() {
        let tmp = tempdir();
        write_file(tmp.path(), "a.txt", "needle here\nhaystack\n");

        let (out, err) = capture(&args("needle", &[&tmp.path().join("a.txt")]));

        assert_eq!(
            out,
            format!(
                "{}\n    1   needle here\n",
                tmp.path().join("a.txt").display()
            )
        );
        assert_eq!(err, "");
    }

    #[test]
    fn running_in_preview_mode_is_case_sensitive_by_default() {
        let tmp = tempdir();
        write_file(tmp.path(), "a.txt", "NEEDLE\n");

        let mut args = args("needle", &[tmp.path()]);
        assert_eq!(capture(&args).0, "");

        args.case_insensitive = true;
        assert!(capture(&args).0.contains("NEEDLE"));
    }

    #[test]
    fn running_in_replace_mode() {
        let tmp = tempdir();
        let hit = write_file(tmp.path(), "hit.txt", "one\n");
        let miss = write_file(tmp.path(), "miss.txt", "nothing\n");

        let mut args = args("one", &[tmp.path()]);
        args.replace = Some(String::from("two"));
        let (out, err) = capture(&args);

        assert_eq!(fs::read_to_string(&hit).unwrap(), "two\n");
        assert_eq!(fs::read_to_string(&miss).unwrap(), "nothing\n");
        assert!(out.contains(&format!("{}\n", hit.display())));
        assert!(out.contains(&format!("{}\n", miss.display())));
        assert!(out.ends_with("1 files changed\n"));
        assert_eq!(err, "");
    }

    #[test]
    fn running_in_replace_mode_without_any_matches() {
        let tmp = tempdir();
        write_file(tmp.path(), "a.txt", "nothing\n");

        let mut args = args("needle", &[tmp.path()]);
        args.replace = Some(String::from("pin"));

        assert!(capture(&args).0.ends_with("No files changed\n"));
    }

    #[test]
    fn running_skips_binary_files_silently() {
        let tmp = tempdir();
        let binary = write_file(tmp.path(), "a.bin", b"needle\0\0\0");

        let mut args = args("needle", &[&binary]);
        assert_eq!(capture(&args), (String::new(), String::new()));

        args.verbose = true;
        assert_eq!(capture(&args).1, format!("{} (binary)\n", binary.display()));
    }

    #[test]
    fn running_reports_files_it_cannot_decode() {
        let tmp = tempdir();
        // Not valid UTF-8, but no NUL bytes either, so it passes the binary check
        let path = write_file(tmp.path(), "latin1.txt", b"caf\xe9 needle\n");

        let (out, err) = capture(&args("needle", &[&path]));

        assert_eq!(out, "");
        assert_eq!(err, format!("{} (read error)\n", path.display()));
    }

    #[test]
    fn running_without_paths_falls_back_to_git() {
        // The crate root is a Git repository, so this covers the `git ls-files` path
        let (out, _) = capture(&args("fn build_regex", &[]));
        assert!(out.contains("src/main.rs"));
    }

    #[test]
    fn running_rejects_invalid_regexes() {
        let mut out: Vec<u8> = Vec::new();
        let mut err: Vec<u8> = Vec::new();
        let result = run(&args("foo(", &[]), &mut out, &mut err);

        assert_eq!(
            format!("{}", result.unwrap_err()),
            "Not a valid regex: `foo(`"
        );
    }

    #[test]
    fn listing_files_from_git() {
        let files = files_from_git().unwrap();
        assert!(files.contains(&PathBuf::from("src/main.rs")));
        assert!(files.contains(&PathBuf::from("Cargo.toml")));
    }

    #[test]
    fn parsing_cli_args() {
        let args = CliArgs::parse_from(["sr", "-s", "foo"]);
        assert_eq!(args.pattern, "foo");
        assert_eq!(args.replace, None);
        assert!(args.paths.is_empty());
        assert!(!args.verbose);
        assert!(!args.case_insensitive);

        let args = CliArgs::parse_from(["sr", "-v", "-i", "-s", "foo", "-r", "bar", "a.txt", "b/"]);
        assert!(args.verbose);
        assert!(args.case_insensitive);
        assert_eq!(args.pattern, "foo");
        assert_eq!(args.replace.as_deref(), Some("bar"));
        assert_eq!(
            args.paths,
            vec![PathBuf::from("a.txt"), PathBuf::from("b/")]
        );

        let args = CliArgs::parse_from([
            "sr",
            "--verbose",
            "--case-insensitive",
            "--search",
            "foo",
            "--replace",
            "bar",
        ]);
        assert!(args.verbose);
        assert!(args.case_insensitive);
        assert_eq!(args.replace.as_deref(), Some("bar"));
    }

    #[test]
    fn parsing_cli_args_requires_a_pattern() {
        assert!(CliArgs::try_parse_from(["sr"]).is_err());
    }

    #[test]
    fn installing_the_ctrlc_handler() {
        // Only one handler can ever be installed per process, so this is the only test that may
        // call it
        setup_ctrlc();
    }
}
