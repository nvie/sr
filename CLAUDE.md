# sr

A mass search & replace CLI, written in Rust. Single binary, single source file:
`src/main.rs` (~200 lines). No library crate, no modules, no tests.

## What it does

`sr -s '<regex>'` previews matches (colored, `file` + `lineno` + highlighted line).
Adding `-r '<replacement>'` rewrites files in place — destructive, no backups.
Without explicit path args it operates on files known to Git (`git ls-files --cached --modified --exclude-standard --deduplicate`); with path args it walks
them via `walkdir`. Binary files are detected with `content_inspector` and never
written.

## Build & run

    cargo build            # debug
    cargo build --release
    cargo run -- -s 'foo'  # note the `--` before sr's own flags

`bin/publish.sh` builds a release tarball and walks the user through creating the
GitHub release + Homebrew tap lines. It is run manually by nvie; never run it.

## Conventions in this codebase

- **Free functions, not types.** Everything is a small top-level `fn` in
  `main.rs`; `main()` orchestrates. There is no struct beyond `CliArgs`. Keep it
  that way unless a change genuinely needs state — don't introduce modules or a
  `lib.rs` for a feature that fits as another `fn`.
- **Predicates read as English**: `is_hidden`, `is_file`, `is_text_file`,
  `include_entry`. New filters follow that naming.
- **Errors: `anyhow`.** Functions that can fail return `Result<T>` (the
  `anyhow::Result` alias) and attach `.with_context(|| ...)` with a
  user-readable message. `main()` returns `Result<()>`.
- **CLI: `clap` 4 derive** on `CliArgs`, short flags spelled explicitly
  (`#[arg(short = 's', long = "search")]`); the struct-level attribute is
  `#[command(...)]`. `PathBuf` fields need no `value_parser` — clap derives one.
- **Output styling: `ansi_term`.** The palette is fixed and meaningful:
  `Blue` = file header, `Purple` = the matched substring, `Fixed(248).dimmed()`
  = de-emphasized (line numbers, unchanged/binary files). Reuse these, don't add
  new colors ad hoc.
- **Preview vs. replace are the two modes**, branched on `args.replace.is_some()`
  in the main loop. Any new behavior has to make sense in both, or be clearly
  scoped to one.
- **Verbose is opt-in noise only.** `-v` adds skipped-file lines to stderr;
  normal operation stays quiet. Results go to stdout, problems to stderr.

## Gotchas

- `files_from_git` passes `-z` to `git ls-files` on purpose: without it git quotes
  and backslash-escapes any name containing a quote, a newline or a non-ASCII
  byte, and `sr` would then look for a file that does not exist. `parse_ls_files`
  splits the NUL-separated output; `path_from_bytes` keeps names byte-exact on
  Unix, where a file name need not be valid UTF-8.
- `is_file` does `entry.metadata().unwrap()` — it will panic on a metadata
  error. If you touch that path, prefer fixing it properly over adding another
  `unwrap`.
- Replacement strings use `regex` crate syntax for groups: `$1`, `${name}`.
- `Cargo.lock` pins `proc-macro2`, `quote`, `syn`, `clap_lex` and `unicode-ident`
  to the last versions that still build on rustc 1.69 (the toolchain in use here).
  A bare `cargo update` on 1.69 will pull versions that refuse to compile; on a
  newer toolchain the pins can simply be lifted.

## Releasing

Version lives in `Cargo.toml` only. A release = bump version, add a section at
the top of `CHANGELOG.md` (`## vX.Y.Z` + one line of prose), then nvie runs
`bin/publish.sh` and updates the Homebrew tap. Don't tag, push, or publish.
