## sr = search + replace

A simple mass **S**earch & **R**eplace tool.

To install:

    $ brew install nvie/tap/sr

## First search, then replace

The idea for `sr` is as follows: run it with just the `-s` (= search) flag
first. You can preview the results and iterate on your regular expression until
it produces the matches you're looking for. Then add the `-r` (replace) flag
to write files.

Unless you specify explicit files/directories, it will only search and replace
in files tracked by Git. It will never write to binary files.

`sr` only runs inside a Git repository, even when you pass explicit paths. That
is on purpose: replacing is destructive, and Git is the undo button.

```sh
$ sr -s 'banana'
```

Check if those results match your expectations, then add an `-r 'replacement'`
string to replace them all:

```sh
$ sr -s 'banana' -r 'mango'
```

To see exactly what those replacements will produce before touching any files,
add `-n` (= dry run). It prints the same preview as `-s`, except it highlights
what each match would _become_ rather than what it matched:

```sh
$ sr -s 'banana' -r 'mango' -n
```

**Please note that this is a destructive operation.** It does not make
a backup[^1]. Please only use this on files checked into Git, and after
checking the preview results (before adding the `-r 'replacement'` flag).

```sh
$ sr -s 'banana' -r 'mango'              # All files known to Git
$ sr -s 'banana' -r 'mango' src/         # All files under src/**
$ sr -s 'banana' -r 'mango' src/**/*.js  # JavaScript files only
$ sr -s 'banana' -r 'mango' README.md    # Just in README.md
$ sr -s 'banana' -r 'mango' .            # All files under this root
```

## Regexes and match groups

The argument to `-s` is a regex. `^` and `$` anchor to the start and end of
a *line*, not of the file, and `-i` makes the match case-insensitive.

Example:

```sh
$ sr -s 'ba(na)+' -r '$1, batman!'
```

Will replace:

- `banana` → `nana, batman!`
- `bananananananana` → `nanananananana, batman!`

**NOTE:** A `$` has special meaning in regexes as well as in most shells.
Therefore, always quote the arguments to `-s` and `-r` with single (`'`)
quotes, so those `$` will not get interpreted by your shell.

Another example, using named match groups:

```sh
$ sr -is '(?P<greet>hi|hello) (?P<name>.*?)!' -r 'Why ${greet}, ${name}!!'
```

Will replace:

- `hi Alice!` → `Why hi, Alice!!`
- `HELLO Bob!` → `Why HELLO, Bob!!`

[^1]: Yet. Perhaps I'll add this feature later.
