extern crate walkdir;
use ansi_term::Colour::{Blue, Purple};
use anyhow::{Context, Result};
use clap::Parser;
use ctrlc::set_handler;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::string::String;
use walkdir::{DirEntry, WalkDir};

#[derive(Parser)]
#[clap(version, about, long_about = None)]
struct CliArgs {
    #[clap(short = 's', long = "search")]
    pattern: String,

    #[clap(short = 'r', long = "replace")]
    replace: Option<String>,

    #[clap(parse(from_os_str), required = true)]
    paths: Vec<PathBuf>,
}

fn setup_ctrlc() {
    // Set automatic Ctrl+C handling
    set_handler(move || {
        println!("received Ctrl+C!");
    })
    .expect("Error setting Ctrl-C handler");
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

fn include_entry(entry: &DirEntry) -> bool {
    is_file(entry) || !is_hidden(entry)
}

fn main() -> Result<()> {
    setup_ctrlc();

    let args = CliArgs::parse();

    // We may want to move this to a native "Cli" arg type? Read the clap docs to see how!
    let re = Regex::new(&args.pattern)
        .with_context(|| format!("Not a valid regex: `{:?}`", args.pattern))?;

    let mut last: String = String::new();

    for path in args.paths {
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

            let content = match std::fs::read_to_string(&path) {
                Err(_) => {
                    eprintln!("Skipping {} (binary?)", pathstr);
                    continue;
                }
                Ok(content) => content,
            };

            for (lineno0, line) in content.lines().enumerate() {
                for m in re.find_iter(line) {
                    if last != pathstr {
                        if !last.is_empty() {
                            // This is the first loop, no separator needed here
                            println!();
                        }
                        println!("{}", Blue.paint(pathstr));
                        last = String::from(pathstr);
                    }

                    let lineno = lineno0 + 1;
                    println!(
                        "{:5}  {}{}{}",
                        lineno,
                        &line[..m.start()],
                        Purple.paint(&line[m.start()..m.end()]),
                        &line[m.end()..]
                    );
                }
            }
        }
    }

    Ok(())
}
