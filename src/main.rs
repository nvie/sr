extern crate walkdir;
use ansi_term::Colour::{Blue, Purple};
use anyhow::{Context, Result};
use clap::Parser;
use ctrlc::set_handler;
use regex::Regex;
use std::path::PathBuf;
use walkdir::WalkDir;

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

fn main() -> Result<()> {
    setup_ctrlc();

    let args = CliArgs::parse();

    // We may want to move this to a native "Cli" arg type? Read the clap docs to see how!
    let re = Regex::new(&args.pattern)
        .with_context(|| format!("Not a valid regex: `{:?}`", args.pattern))?;

    for path in args.paths {
        for file in WalkDir::new(path)
            .into_iter()
            .filter_entry(|entry| {
                let metadata = entry.metadata().unwrap();
                !metadata.is_file()
                    || (entry
                        .path()
                        .file_name()
                        .map(|basename| match basename.to_str() {
                            None => false,
                            Some(s) => !(s.starts_with(".")),
                        })
                        .is_some())
            })
            .filter_map(|file| file.ok())
        {
            if file.metadata().unwrap().is_file() {
                let path = file.path();

                let pathstr: &str = file
                    .path()
                    .to_str()
                    .with_context(|| format!("Error in file name"))?;

                let content = match std::fs::read_to_string(&path) {
                    Err(_) => {
                        eprintln!("Skipping {} (binary?)", pathstr);
                        continue;
                    }
                    Ok(content) => content,
                };

                println!("{}", Blue.paint(pathstr));
                for (lineno0, line) in content.lines().enumerate() {
                    for m in re.find_iter(line) {
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
                println!("");
            }
        }
    }

    // for root in args.paths {
    //     let dir = PathDir::new(root)?;
    //     for entry in dir.walk() {
    //         match PathType::from_entry(entry?)? {
    //             PathType::File(file) => {

    // }
    // }
    // }
    // }

    Ok(())
}
