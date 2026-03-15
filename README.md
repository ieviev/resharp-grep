# re

grep finds one thing at a time. `re` finds where multiple things appear together - on the same line, in the same paragraph, within N lines of each other, or anywhere in the same file. powered by [RE#](https://github.com/ieviev/resharp).

[install](#install) | [web playground](https://ieviev.github.io/resharp-webapp/)

## why re

```sh
# find unsafe code that unwraps within 5 lines - potential panics in unsafe blocks
re --near 5 --and unsafe --and unwrap src/

# find errors mentioning timeout, but filter out debug and trace noise
re error --not debug --not trace src/

# list files that use both tokio and diesel - mixed async/sync code
re --scope file --and tokio --and diesel src/

# find paragraphs that mention both password and plaintext - credential exposure
re -p password -p plaintext .

# search within YAML sections for entries that have both host and port
re --scope '---' --and host --and port config/

# find markdown sections that discuss both API and deprecation
re --scope '\n## ' --and API --and deprecated docs/
```

with grep, each of these would require chaining multiple commands and losing file context, line numbers, and highlighting. `re` does it in one shot, and faster than any other tool out there.

## quick start

`re` works similar to ripgrep for simple searches:

```sh
re TODO src/                              # find TODO in src/
re -i error .                             # case-insensitive search
re -F 'std::io' src/                      # literal string (no regex)
```

the difference shows up when you need more than one term.

## boolean constraints

`-a` requires a term, `-N` excludes one. all terms must co-occur within the current scope (line by default).

```sh
# lines containing both "error" and "timeout"
re -a error -a timeout src/

# error but not debug - filter out noisy log lines
re error -N debug .

# lines with both literal strings (no regex interpretation)
re -F 'std::io' -F 'Error' src/
```

| flag | effect | compiles to |
|------|--------|-------------|
| `-a` / `--and` | match must also contain this pattern | `&(_*pattern_*)` |
| `-N` / `--not` | match must not contain this pattern | `&~(_*pattern_*)` |
| `-F` / `--lit` | match must contain this literal string | `&(_*literal_*)` |

## scope

by default, all terms must appear on the same line. `-d` (delimiter/scope) widens the window.

| scope | flag | terms must appear within |
|-------|------|--------------------------|
| line | (default) | a single line |
| paragraph | `-p` | a text block separated by blank lines |
| file | `-d file` | anywhere in the same file (lists filenames) |
| proximity | `--near N` | N lines of each other |
| custom | `-d '<delim>'` | text between occurrences of the delimiter |

```sh
# find text blocks that mention both "error" and "retry"
re -p error -p retry logs/

# list files that import both serde and async-trait
re -d file -a serde -a async-trait src/

# find markdown sections discussing both API and deprecation
re -d '\n## ' -a API -a deprecated docs/

# find TODO and FIXME within 3 lines of each other - related work items
re --near 3 -a TODO -a FIXME .
```

`-p word` is shorthand for `--scope paragraph -a word`.

## the RE# pattern language

the flags are convenient shorthands, but you can also write the whole pattern as a single positional argument using boolean operators directly:

| operator | meaning | example |
|----------|---------|---------|
| `&` | intersection - both sides must match | `(error)&(timeout)` matches lines with both |
| `~(...)` | complement - must not match | `~(_*debug_*)` excludes lines containing debug |
| `_` | wildcard - any character including newlines | `_*error_*` matches error anywhere in a block |

```sh
# these are equivalent:
re error -N debug .
re '(_*error_*)&~(_*debug_*)' .

# patterns that go beyond what flags can express:
# hex strings that contain both a digit and a letter
re '([0-9a-f]+)&(_*[0-9]_*)&(_*[a-f]_*)'
```

use `\_` for a literal underscore, `-R` for standard regex mode, or `-F` for fixed strings.

try patterns interactively in the [web playground](https://ieviev.github.io/resharp-webapp/).

## differences from ripgrep

most ripgrep flags work the same. the key differences:

| ripgrep | re | notes |
|---------|-----|--------|
| `-a` processes binary files | `-a` means `--and` (require term) | use `-uuu` for binary file processing |
| `_` is a literal character | `_` is a wildcard | use `-R` or `\_` for literal underscore |
| standard regex only | adds `&`, `~`, `_` operators | use `-R` to disable |

## exit codes

`0` match found, `1` no match, `2` error

## install

### cargo

```sh
cargo install resharp-grep  # installs binary named `re`
```

### prebuilt binaries

download from [GitHub releases](https://github.com/ieviev/resharp-grep/releases).

### nix

```sh
nix profile install github:ieviev/resharp-grep
```

or in a flake:

```nix
inputs.resharp.url = "github:ieviev/resharp-grep";
```

nix package includes shell completions.

## how it works

all flags compile down to [RE#](https://github.com/ieviev/resharp) patterns. for example:

```sh
re --near 5 -a unsafe -a unwrap
```

compiles to:

```
(_*unsafe_*) & (_*unwrap_*) & ~((_*\n_*){5})
```

each `-a` term becomes an intersection, `--near 5` rejects spans containing 5+ newlines, and scopes add their own boundary. because everything shares the same pattern representation, all output modes (highlighting, context, `--count`, `--json`, etc.) work uniformly.

see the [RE# engine](https://github.com/ieviev/resharp) for more on the pattern language.

Have fun!
