## sr

A simple mass **s**earch & **r**eplace tool.

To install:

    $ brew install sr

## Usage

Use the `-s <pattern>` flag to search for any regex in your entire
codebase:

```sh
$ sr -s 'banana'
```

Check if those results match your expectations, then add an `-r
'replacement'` string to replace them all:

```sh
$ sr -s 'banana' -r 'mango'
```

When in a Git repo, `sr` will only touch and search & replace files known
by Git, or you can specify which files or directories to (recursively)
search and replace in:

```sh
$ sr -s 'banana' -r 'mango'              # All files known to Git
$ sr -s 'banana' -r 'mango' src/         # All files under src/**
$ sr -s 'banana' -r 'mango' src/**/*.js  # JavaScript files only
$ sr -s 'banana' -r 'mango' README.md    # Just in README.md
$ sr -s 'banana' -r 'mango' .            # All files under this root
```
