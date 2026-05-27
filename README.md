# re

grep finds one thing at a time. `re` finds where multiple things appear together: on the same line, in the same paragraph, within N lines of each other, or anywhere in the same file. powered by [RE#](https://github.com/ieviev/resharp).

[install](#install) | [web playground](https://ieviev.github.io/resharp-webapp/)

> flags are not a drop-in match for grep/rg. some short flags are repurposed to expose RE# operators (e.g. `-a`, `-N`, `-S`, `-s`, `-d`, `-p`, `-f`, `-M`).

## why re

```sh
# find unsafe code that unwraps within 5 lines - potential panics in unsafe blocks
re --near 5 -a unsafe -a unwrap src/

# find errors mentioning timeout, but filter out debug and trace noise
re error -N debug -N trace src/

# list files that use both tokio and diesel - mixed async/sync code
re -f -a tokio -a diesel src/

# search within YAML sections for entries that have both host and port
re -S '---' -a host -a port config/

# find markdown sections that discuss both API and deprecation
re -S '\n## ' -a API -a deprecated docs/
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

## constraints

`-a` requires a term, `-N` excludes one. all terms must co-occur within the same **scope** (same line by default).

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
| `-N` / `--not` | drop the line/scope if it contains this pattern | `&~(_*pattern_*)` anchored to scope |
| `-e` | union - match any of the patterns | `(pat1)\|(pat2)` |
| `-F` / `--lit` | match must contain this literal string | `&(_*literal_*)` |

## scope

the scope is the window that `-a` / `-N` terms must share. by default it is a single line. widen it to a paragraph, a proximity window, a whole file, or a custom delimiter.

| scope | flag | window |
|-------|------|--------|
| line | (default) | one line |
| paragraph | `-p` | a block separated by blank lines |
| proximity | `--near N` | any N consecutive lines |
| file | `-f` | the entire file (prints filenames) |
| custom | `-S PAT` | text between occurrences of `PAT` |
| gated | `-S PAT -s SUB` | only `-S` scopes that also contain `SUB` |

examples:

```sh
re -p -a error -a retry logs/                       # paragraph mentions both
re --near 3 -a TODO -a FIXME .                      # within 3 lines
re -f -a serde -a async-trait src/                  # same file
re -S '\n## ' -a API -a deprecated docs/            # markdown sections
re -S '\n## ' -s deprecated -a API -a removed docs/ # only deprecated sections
```

notes:
- `-S` delimits the search window. `-d PAT` is different: it delimits the match span (the engine emits regions free of `PAT`). see the section above.
- `-p` is shorthand for `--scope paragraph`.
- use `--show-pattern` to inspect the compiled RE# for any scope combination.

## raw patterns

the flags are convenient shorthands, but you can also write patterns directly using the [RE#](https://github.com/ieviev/resharp) operators:

| operator | meaning | example |
|----------|---------|---------|
| `&` | intersection - both sides must match | `(error)&(timeout)` matches lines with both |
| `~(...)` | complement - must not match | `~(.*debug.*)` excludes lines containing debug |

```sh
# these are equivalent:
re error -N debug .
re '(.*error.*)&~(.*debug.*)' .

# patterns that go beyond what flags can express:
# hex strings that contain both a digit and a letter
re '([0-9a-f]+)&(.*[0-9].*)&(.*[a-f].*)'
```

use `-F` for fixed strings.

try patterns interactively in the [web playground](https://ieviev.github.io/resharp-webapp/).

## differences from ripgrep

most ripgrep flags work the same. the key differences:

| ripgrep | re | notes |
|---------|-----|--------|
| `-a` processes binary files | `-a` means `--and` (require term) | use `-uuu` for binary file processing |
| standard regex only | RE# regex with `&` (intersection) and `~` (complement) | superset of standard regex |

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
