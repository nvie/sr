use ansi_term::Colour::{Blue, Fixed, Purple};
use anyhow::{Context, Result};
use clap::Parser;
use content_inspector::inspect;
use ctrlc::set_handler;
use regex::{Regex, RegexBuilder};
use std::fs;
use std::io::Read;
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

fn main() -> Result<()> {
    setup_ctrlc();

    let args = CliArgs::parse();

    // We may want to move this to a native "Cli" arg type? Read the clap docs to see how!
    let re = RegexBuilder::new(&args.pattern)
        .case_insensitive(args.case_insensitive)
        .build()
        .with_context(|| format!("Not a valid regex: `{}`", &args.pattern))?;

    let mut last: String = String::new();

    let all_paths: Vec<PathBuf> = if !args.paths.is_empty() {
        args.paths
    } else {
        match files_from_git() {
            Ok(paths) => paths,
            Err(_) => {
                panic!("No input files specified")
            }
        }
    };

    let mut count = 0;

    for path in all_paths {
        for file in WalkDir::new(path)
            .into_iter()
            .filter_entry(include_entry)
            .filter_map(|file| file.ok())
        {
            if !is_file(&file) {
                continue;
            }

            let path = file.path();
            let pathstr: &str = file
                .path()
                .to_str()
                .with_context(|| "Error in file name".to_string())?;

            if !is_text_file(path) {
                if args.verbose {
                    eprintln!(
                        "{} (binary)",
                        Fixed(248).dimmed().paint(pathstr.to_string())
                    );
                }
                continue;
            }

            let content = match fs::read_to_string(&path) {
                Err(_) => {
                    eprintln!("{} (read error)", pathstr);
                    continue;
                }
                Ok(content) => content,
            };

            if let Some(ref replacement) = args.replace {
                // Inline-replace matches
                let new_content = re.replace_all(&content, replacement);
                if content != new_content {
                    fs::write(path, &*new_content)
                        .with_context(|| format!("Unable to write to file `{}`", path.display()))?;
                    count += 1;
                    println!("{}", path.display());
                } else {
                    println!(
                        "{}",
                        Fixed(248).dimmed().paint(format!("{}", path.display()))
                    );
                }
            } else {
                // Highlight matches
                for (lineno0, line) in content.lines().enumerate() {
                    let lineno = lineno0 + 1;

                    let highlighted: Option<String> = highlight_matches(&re, line);
                    if let Some(hline) = highlighted {
                        if last != pathstr {
                            if !last.is_empty() {
                                // Print separator unless this is the first loop
                                println!();
                            }
                            println!("{}", Blue.paint(pathstr));
                            last = String::from(pathstr);
                        }

                        println!(
                            "{}   {}",
                            Fixed(248).dimmed().paint(format!("{:5}", lineno)),
                            hline
                        );
                    }
                }
            }
        }
    }

    if args.replace.is_some() {
        if count > 0 {
            println!("{} files changed", count);
        } else {
            println!("No files changed");
        }
    }

    Ok(())
}
