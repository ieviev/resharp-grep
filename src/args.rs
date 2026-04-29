use std::io::IsTerminal;
use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "re",
    about = "recursive search with boolean constraints",
    version,
    disable_help_flag = true,
    before_help = "\x1b[1mExamples:\x1b[0m
  re TODO src/                              find TODO in src/
  re error -N debug -N trace src/           errors, filtering out debug and trace noise
  re --near 5 -a unsafe -a unwrap src/      unsafe code that unwraps within 5 lines
  re -d file -a tokio -a diesel src/        files using both tokio and diesel
  re -p password -p plaintext .             paragraphs mentioning both password and plaintext
  re -d '\\n## ' -a API -a deprecated docs/ markdown sections discussing both API and deprecation
  re -d '---' -a host -a port config/       YAML sections with both host and port
  re --json TODO src/                       JSON output, one object per match",
    after_help = "see https://github.com/ieviev/resharp for the pattern language."
)]
pub struct Args {
    /// regex pattern (positional, mutually exclusive with -e/-f)
    #[arg(value_name = "PATTERN")]
    pub pattern: Option<String>,

    /// paths to search (files or directories)
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// regex pattern (repeatable, combined with |)
    #[arg(short = 'e', long = "regexp", value_name = "PATTERN")]
    pub regexp: Vec<String>,

    /// file containing patterns, one per line
    #[arg(short = 'f', long = "file", value_name = "FILE")]
    pub pattern_file: Vec<PathBuf>,

    /// case insensitive search
    #[arg(short = 'i', long = "ignore-case")]
    pub ignore_case: bool,

    /// case sensitive search (overrides -i)
    #[arg(short = 's', long = "case-sensitive")]
    pub case_sensitive: bool,

    /// smart case (insensitive unless pattern has uppercase)
    #[arg(short = 'S', long = "smart-case")]
    pub smart_case: bool,

    /// invert match (show non-matching lines)
    #[arg(short = 'v', long = "invert-match")]
    pub invert_match: bool,

    /// match only whole words
    #[arg(short = 'w', long = "word-regexp")]
    pub word_regexp: bool,

    /// match only whole lines
    #[arg(short = 'x', long = "line-regexp")]
    pub line_regexp: bool,

    /// stop after NUM matches per file
    #[arg(short = 'm', long = "max-count", value_name = "NUM")]
    pub max_count: Option<usize>,

    /// suppress output, exit 0 if match found
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// print count of matches per file
    #[arg(short = 'c', long = "count")]
    pub count: bool,

    /// print only filenames with matches
    #[arg(short = 'l', long = "files-with-matches")]
    pub files_with_matches: bool,

    /// print only filenames without matches
    #[arg(long = "files-without-match")]
    pub files_without_match: bool,

    /// print only matched parts
    #[arg(short = 'o', long = "only-matching")]
    pub only_matching: bool,

    /// show line numbers
    #[arg(short = 'n', long = "line-number")]
    pub line_number: bool,

    /// suppress line numbers
    #[arg(long = "no-line-number")]
    pub no_line_number: bool,

    /// show column number of first match
    #[arg(long = "column")]
    pub column: bool,

    /// show byte offset of match
    #[arg(short = 'b', long = "byte-offset")]
    pub byte_offset: bool,

    /// literal string mode, or repeatable literal constraints (-F lit1 -F lit2)
    #[arg(short = 'F', long = "fixed-strings", aliases = ["fixed", "lit"], num_args = 0..=1, action = clap::ArgAction::Append, value_name = "STRING")]
    pub fixed_strings: Option<Vec<String>>,

    /// standard regex mode (disables &, ~, and _ operators)
    #[arg(short = 'R', long = "raw")]
    pub raw: bool,

    /// require PATTERN to also appear in match (repeatable)
    #[arg(short = 'a', long = "and", visible_short_alias = 'W', visible_alias = "with", alias = "add", value_name = "PATTERN")]
    pub and: Vec<String>,

    /// exclude matches containing PATTERN (repeatable)
    #[arg(short = 'N', long = "not", value_name = "PATTERN")]
    pub not: Vec<String>,

    /// exclude PATTERN from match body itself (no scope anchoring, repeatable)
    #[arg(short = 'E', long = "exclude-match", value_name = "PATTERN")]
    pub exclude_match: Vec<String>,

    /// replace matches with TEXT in output
    #[arg(long = "replace", value_name = "TEXT")]
    pub replace: Option<String>,

    /// lines of context after match
    #[arg(short = 'A', long = "after-context", value_name = "NUM")]
    pub after_context: Option<usize>,

    /// lines of context before match
    #[arg(short = 'B', long = "before-context", value_name = "NUM")]
    pub before_context: Option<usize>,

    /// lines of context before and after match
    #[arg(short = 'C', long = "context", value_name = "NUM")]
    pub context: Option<usize>,

    /// color output: auto, always, never
    #[arg(long = "color", value_name = "WHEN", default_value = "auto")]
    pub color: String,

    /// group results by file (default for terminal)
    #[arg(long = "heading")]
    pub heading: bool,

    /// don't group results by file
    #[arg(long = "no-heading")]
    pub no_heading: bool,

    /// include/exclude glob (repeatable, prefix with ! to exclude)
    #[arg(short = 'g', long = "glob", value_name = "GLOB")]
    pub glob: Vec<String>,

    /// case-insensitive glob
    #[arg(long = "iglob", value_name = "GLOB")]
    pub iglob: Vec<String>,

    /// alias for -g GLOB (ripgrep/grep compatibility)
    #[arg(long = "include", value_name = "GLOB", hide = true)]
    pub include: Vec<String>,

    /// alias for -g !GLOB (ripgrep/grep compatibility)
    #[arg(long = "exclude", value_name = "GLOB", hide = true)]
    pub exclude: Vec<String>,

    /// search only files matching TYPE
    #[arg(short = 't', long = "type", value_name = "TYPE")]
    pub file_type: Vec<String>,

    /// exclude files matching TYPE
    #[arg(short = 'T', long = "type-not", value_name = "TYPE")]
    pub type_not: Vec<String>,

    /// list files matching globs/types (no content search)
    #[arg(long = "files")]
    pub files: bool,

    /// list available file types
    #[arg(long = "type-list")]
    pub type_list: bool,

    /// search hidden files and directories
    #[arg(long = "hidden")]
    pub hidden: bool,

    /// don't respect ignore files
    #[arg(long = "no-ignore")]
    pub no_ignore: bool,

    /// don't respect .gitignore
    #[arg(long = "no-ignore-vcs")]
    pub no_ignore_vcs: bool,

    /// additional ignore file (like .gitignore)
    #[arg(long = "ignore-file", value_name = "PATH")]
    pub ignore_file: Vec<PathBuf>,

    /// reduce filtering (-u: hidden, -uu: +no-ignore, -uuu: +binary)
    #[arg(short = 'u', long = "unrestricted", action = clap::ArgAction::Count)]
    pub unrestricted: u8,

    /// follow symbolic links
    #[arg(short = 'L', long = "follow")]
    pub follow: bool,

    /// number of search threads (0 = auto)
    #[arg(short = 'j', long = "threads", value_name = "NUM")]
    pub threads: Option<usize>,

    /// max directory depth
    #[arg(long = "max-depth", value_name = "NUM")]
    pub max_depth: Option<usize>,

    /// skip files larger than NUM bytes (supports K, M, G suffixes)
    #[arg(long = "max-filesize", value_name = "NUM+SUFFIX?")]
    pub max_filesize: Option<String>,

    /// sort results by path
    #[arg(long = "sort", value_name = "CRITERION")]
    pub sort: Option<String>,

    /// allow matches to span multiple lines
    #[arg(long = "multiline")]
    pub multiline: bool,

    /// paragraph scope, or repeatable word intersection (-p word1 -p word2)
    #[arg(short = 'p', long = "paragraphs", alias = "§", num_args = 0..=1, action = clap::ArgAction::Append, value_name = "WORD")]
    pub paragraphs: Option<Vec<String>>,

    /// use memory-mapped I/O (auto: mmap files >= 1MB)
    #[arg(long = "mmap")]
    pub mmap: bool,

    /// disable memory-mapped I/O
    #[arg(long = "no-mmap")]
    pub no_mmap: bool,

    /// max DFA state capacity (default: 65535)
    #[arg(long = "dfa-capacity", value_name = "NUM", default_value = "65535")]
    pub dfa_capacity: usize,

    /// DFA precompilation threshold (default: 0)
    #[arg(long = "dfa-threshold", value_name = "NUM", default_value = "0")]
    pub dfa_threshold: usize,

    /// generate shell completions (bash, zsh, fish, elvish, powershell)
    #[arg(long = "completions", value_name = "SHELL", hide = true)]
    pub completions: Option<clap_complete::Shell>,

    /// scope for intersection (line, paragraph, file, or a boundary regex)
    #[arg(short = 'd', long = "scope", value_name = "SCOPE")]
    pub scope: Option<String>,

    /// find patterns within N lines of each other (use with --and)
    #[arg(short = 'P', long = "near", value_name = "NUM")]
    pub near: Option<usize>,

    /// stop after NUM total matches across all files
    #[arg(short = 'M', long = "max-total", value_name = "NUM")]
    pub max_total: Option<usize>,

    /// deduplicate matched strings (useful with -o)
    #[arg(long = "unique")]
    pub unique: bool,

    /// print summary stats (files, matches, lines, time)
    #[arg(long = "stats")]
    pub stats: bool,

    /// NUL byte as separator (for xargs -0)
    #[arg(short = '0', long = "null")]
    pub null: bool,

    /// run CMD on each matched file ({} = path, appended if absent)
    #[arg(long = "exec", value_name = "CMD")]
    pub exec: Option<String>,

    /// output results as JSON (one object per match line)
    #[arg(long = "json")]
    pub json: bool,

    /// show the enclosing function or block header for each match
    #[arg(long = "show-scope")]
    pub show_scope: bool,

    /// truncate long lines to fit terminal width
    #[arg(long = "trim")]
    pub trim: bool,

    /// output file:line:col:text for editor integration (one line per match)
    #[arg(long = "vimgrep")]
    pub vimgrep: bool,

    /// limit total output lines; prints a truncation notice when cut
    #[arg(long = "head", value_name = "N")]
    pub head: Option<usize>,

    /// skip first N output lines (for tool integration, pairs with --head)
    #[arg(long = "offset", value_name = "N")]
    pub offset: Option<usize>,

    /// print only the total match count as a plain number (machine-readable)
    #[arg(long = "count-matches")]
    pub count_matches: bool,

    /// print help
    #[arg(long = "help", action = clap::ArgAction::Help)]
    pub help: Option<bool>,

    /// suppress path prefix in output (non-heading mode)
    #[arg(short = 'h', long = "no-filename")]
    pub no_filename: bool,

    /// print all lines, highlighting matches
    #[arg(long = "passthru")]
    pub passthru: bool,

    /// skip lines longer than NUM columns
    #[arg(long = "max-columns", value_name = "NUM")]
    pub max_columns: Option<usize>,

    /// separator between context groups (default: --)
    #[arg(long = "context-separator", value_name = "SEP", default_value = "--")]
    pub context_separator: String,
}

impl Args {
    pub fn is_fixed_strings(&self) -> bool {
        matches!(&self.fixed_strings, Some(v) if v.is_empty())
    }

    fn fixed_words(&self) -> &[String] {
        match &self.fixed_strings {
            Some(words) => words.as_slice(),
            None => &[],
        }
    }

    pub fn is_paragraph_mode(&self) -> bool {
        self.paragraphs.is_some()
    }

    pub fn engine_opts(&self) -> resharp::RegexOptions {
        resharp::RegexOptions {
            dfa_threshold: self.dfa_threshold,
            max_dfa_capacity: self.dfa_capacity,
            ..Default::default()
        }
    }

    pub fn effective_scope(&self) -> &str {
        if let Some(ref scope) = self.scope {
            scope.as_str()
        } else if self.is_paragraph_mode() {
            "paragraph"
        } else if self.multiline || self.near.is_some() {
            "multiline"
        } else {
            "line"
        }
    }

    /// returns (pattern, file_scope_not_patterns)
    pub fn resolve_pattern(&self) -> anyhow::Result<(String, Vec<String>)> {
        let para: Vec<&str> = match &self.paragraphs {
            Some(w) => w.iter().map(|s| s.as_str()).collect(),
            None => vec![],
        };
        // -a is a modifier when a base pattern exists, standalone otherwise
        let has_base = self.pattern.is_some()
            || !self.regexp.is_empty()
            || !self.pattern_file.is_empty();
        let words: Vec<&str> = if has_base {
            para
        } else {
            para.into_iter().chain(self.and.iter().map(|s| s.as_str())).collect()
        };
        let fixed = self.fixed_words();
        let total_terms = words.len() + fixed.len();

        if total_terms > 0 {
            if !self.regexp.is_empty() || !self.pattern_file.is_empty() {
                anyhow::bail!("-e/-f cannot be combined with -W/-p/-F word patterns");
            }
            if self.near.is_some() && total_terms < 2 {
                anyhow::bail!("--near requires at least 2 terms (use -W, --and, or & in pattern)");
            }
            let (pattern, file_nots) = self.build_words_pattern(&words);
            return Ok((pattern, file_nots));
        }

        let mut patterns = Vec::new();

        // -e patterns
        patterns.extend(self.regexp.iter().cloned());

        // -f patterns
        for path in &self.pattern_file {
            let contents = std::fs::read_to_string(path)
                .map_err(|e| anyhow::anyhow!("failed to read pattern file {}: {e}", path.display()))?;
            for line in contents.lines() {
                if !line.is_empty() {
                    patterns.push(line.to_string());
                }
            }
        }

        // positional pattern (only if no -e/-f)
        if patterns.is_empty() {
            match &self.pattern {
                Some(p) => patterns.push(p.clone()),
                None => {
                    if self.exclude_match.is_empty() {
                        anyhow::bail!("no pattern provided");
                    }
                    // -E alone: body defaults to _*, narrowed by apply_excludes
                    patterns.push("_*".to_string());
                }
            }
        }

        // apply transformations
        let mut patterns: Vec<String> = patterns
            .into_iter()
            .map(|p| self.wrap_pattern(p, None))
            .collect();

        // combine with union
        let combined = if patterns.len() == 1 {
            patterns.pop().unwrap()
        } else {
            patterns
                .into_iter()
                .map(|p| format!("({p})"))
                .collect::<Vec<_>>()
                .join("|")
        };

        // apply --and / --not intersection/complement
        let (combined, file_nots) = self.apply_and_not(combined);

        // --near requires at least 2 terms
        if self.near.is_some() && self.and.is_empty() {
            anyhow::bail!("--near requires at least 2 terms (use -W, --and, or & in pattern)");
        }

        // apply -E body excludes (no scope anchoring)
        let combined = self.apply_excludes(combined);

        // apply scope boundary
        let combined = self.apply_scope_boundary(combined);

        Ok((combined, file_nots))
    }

    /// alternation of positive terms for highlighting, None if unnecessary
    pub fn resolve_highlight_pattern(&self) -> Option<String> {
        let has_constraints = !self.and.is_empty()
            || !self.not.is_empty()
            || self.paragraphs.as_ref().map_or(false, |v| !v.is_empty())
            || self.fixed_strings.as_ref().map_or(false, |v| !v.is_empty())
            || self.effective_scope() != "line";

        if !has_constraints {
            return None;
        }

        let mut terms = Vec::new();

        if let Some(ref p) = self.pattern {
            terms.push(self.wrap_pattern(p.clone(), None));
        }
        for p in &self.regexp {
            terms.push(self.wrap_pattern(p.clone(), None));
        }
        for a in &self.and {
            terms.push(self.wrap_pattern(a.clone(), None));
        }
        if let Some(ref words) = self.paragraphs {
            for w in words {
                terms.push(self.wrap_pattern(w.clone(), None));
            }
        }
        for f in self.fixed_words() {
            terms.push(resharp::escape(f));
        }

        if terms.is_empty() {
            return None;
        }
        if terms.len() == 1 {
            Some(terms.pop().unwrap())
        } else {
            Some(terms.into_iter().map(|t| format!("({t})")).collect::<Vec<_>>().join("|"))
        }
    }

    fn build_words_pattern(&self, words: &[&str]) -> (String, Vec<String>) {
        let fixed = self.fixed_words();
        let single = words.len() + fixed.len() == 1
            && self.not.is_empty()
            && self.effective_scope() == "line";

        let mut terms: Vec<String> = words
            .iter()
            .map(|w| self.wrap_pattern(w.to_string(), if single { None } else { Some("_*") }))
            .collect();

        // -F adds literal string constraints (pre-escaped)
        for f in fixed {
            if single {
                terms.push(resharp::escape(f));
            } else {
                terms.push(format!("(_*{escaped}_*)", escaped = resharp::escape(f)));
            }
        }

        let mut combined = terms.join("&");
        let scope = self.effective_scope();
        let wild = if scope == "line" { ".*" } else { "_*" };
        let file_nots = self.apply_nots(&mut combined, scope, wild);

        // line scope needs anchoring when negation complements were baked in
        if file_nots.is_empty() && !self.not.is_empty() && scope == "line" {
            combined = format!("^({combined})$");
        }

        let combined = self.apply_excludes(combined);

        (self.apply_scope_boundary(combined), file_nots)
    }

    fn apply_excludes(&self, combined: String) -> String {
        if self.exclude_match.is_empty() {
            return combined;
        }
        let mut combined = combined;
        for e in &self.exclude_match {
            let term = self.wrap_pattern(e.clone(), Some("_*"));
            combined = format!("({combined})&~{term}");
        }
        combined
    }

    fn apply_and_not(&self, mut combined: String) -> (String, Vec<String>) {
        let has_and = !self.and.is_empty();
        let has_not = !self.not.is_empty();
        if !has_and && !has_not {
            return (combined, vec![]);
        }

        let scope = self.effective_scope();
        let wild = if scope == "line" { ".*" } else { "_*" };

        // file scope with only negations: extract as post-filters, leave pattern unchanged
        if scope == "file" && has_not && !has_and {
            let nots = self.not.iter()
                .map(|n| self.wrap_pattern(n.clone(), Some(wild)))
                .collect();
            return (combined, nots);
        }

        combined = format!("({wild}({combined}){wild})");
        for a in &self.and {
            let term = self.wrap_pattern(a.clone(), Some(wild));
            combined = format!("{combined}&{term}");
        }

        let file_nots = self.apply_nots(&mut combined, scope, wild);

        if scope == "line" {
            combined = format!("^({combined})$");
        }

        (combined, file_nots)
    }

    fn apply_nots(&self, combined: &mut String, scope: &str, wild: &str) -> Vec<String> {
        if self.not.is_empty() {
            return vec![];
        }

        let nots: Vec<String> = self.not.iter()
            .map(|n| self.wrap_pattern(n.clone(), Some(wild)))
            .collect();

        if scope == "file" {
            return nots;
        }

        for term in &nots {
            *combined = format!("{combined}&~{term}");
        }

        vec![]
    }

    fn apply_scope_boundary(&self, combined: String) -> String {
        let scoped = match self.effective_scope() {
            "line" => format!("({combined})&(.*)"),
            "paragraph" => format!("({combined})&((?<=\\A|\n\n)(_*&~(_*\n\n_*)&~(\n_*|_*\n))(?=\n\n|\n\\z|\\z))"),
            "file" | "multiline" => combined,
            custom => format!("({combined})&~(_*{custom}_*)"),
        };
        self.apply_near(scoped)
    }

    fn apply_near(&self, combined: String) -> String {
        match self.near {
            Some(n) => format!("({combined})&~((_*\n_*){{{n}}})"),
            None => combined,
        }
    }

    fn wrap_pattern(&self, mut pattern: String, wild: Option<&str>) -> String {
        if self.is_fixed_strings() {
            pattern = resharp::escape(&pattern);
        } else if self.raw {
            pattern = Self::escape_resharp(&pattern);
        }

        if self.word_regexp {
            pattern = format!(r"\b({pattern})\b");
        }

        if self.line_regexp {
            pattern = format!("^({pattern})$");
        }

        if self.should_ignore_case(&pattern) {
            pattern = format!("(?i){pattern}");
        }

        if let Some(w) = wild {
            let prefix = if pattern.starts_with('^') { "" } else { w };
            let suffix = if pattern.ends_with('$') { "" } else { w };
            // inner parens guard against alternation binding across the wildcards (e.g. a|b -> _*(a|b)_*)
            pattern = format!("({prefix}({pattern}){suffix})");
        }

        pattern
    }

    fn escape_resharp(pattern: &str) -> String {
        let mut out = String::with_capacity(pattern.len());
        let mut chars = pattern.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                out.push('\\');
                if chars.peek().is_some() {
                    out.push(chars.next().unwrap());
                }
            } else if c == '_' || c == '&' || c == '~' {
                out.push('\\');
                out.push(c);
            } else {
                out.push(c);
            }
        }
        out
    }

    fn should_ignore_case(&self, pattern: &str) -> bool {
        if self.case_sensitive {
            return false;
        }
        if self.ignore_case {
            return true;
        }
        if self.smart_case {
            return !pattern.chars().any(|c| c.is_uppercase());
        }
        false
    }

    pub fn color_choice(&self) -> termcolor::ColorChoice {
        if self.json || std::env::var_os("NO_COLOR").is_some() {
            return termcolor::ColorChoice::Never;
        }
        match self.color.as_str() {
            "always" => termcolor::ColorChoice::Always,
            "never" => termcolor::ColorChoice::Never,
            _ => {
                if std::io::stdout().is_terminal() {
                    termcolor::ColorChoice::Auto
                } else {
                    termcolor::ColorChoice::Never
                }
            }
        }
    }

    pub fn effective_hidden(&self) -> bool {
        self.hidden || self.unrestricted >= 1
    }

    pub fn effective_no_ignore(&self) -> bool {
        self.no_ignore || self.unrestricted >= 2
    }

    pub fn search_binary(&self) -> bool {
        self.unrestricted >= 3
    }

    pub fn show_line_number(&self, multi_file: bool) -> bool {
        if self.no_line_number {
            return false;
        }
        self.line_number || multi_file
    }

    pub fn show_heading(&self) -> bool {
        if self.json {
            return false;
        }
        if self.no_heading {
            return false;
        }
        self.heading || std::io::stdout().is_terminal()
    }

    pub fn after_ctx(&self) -> usize {
        self.after_context.or(self.context).unwrap_or(0)
    }

    pub fn before_ctx(&self) -> usize {
        self.before_context.or(self.context).unwrap_or(0)
    }

    pub fn use_mmap(&self, file_size: u64) -> bool {
        if self.no_mmap {
            return false;
        }
        if self.mmap {
            return true;
        }
        file_size >= 1024 * 1024
    }

    pub fn resolved_paths(&self) -> anyhow::Result<Vec<PathBuf>> {
        let paths = if self.paths.is_empty() {
            vec![".".into()]
        } else {
            self.paths.clone()
        };
        for p in &paths {
            if !p.exists() {
                anyhow::bail!("{}: no such file or directory", p.display());
            }
        }
        Ok(paths)
    }

    pub fn effective_max(&self, total_so_far: usize) -> Option<usize> {
        match (self.max_count, self.max_total) {
            (Some(mc), Some(mt)) => Some(mc.min(mt.saturating_sub(total_so_far))),
            (Some(mc), None) => Some(mc),
            (None, Some(mt)) => Some(mt.saturating_sub(total_so_far)),
            (None, None) => None,
        }
    }

    pub fn parse_max_filesize(&self) -> anyhow::Result<Option<u64>> {
        let s = match self.max_filesize.as_ref() {
            Some(s) => s,
            None => return Ok(None),
        };
        let s = s.trim();
        const SUFFIXES: &[(&str, u64)] = &[
            ("k", 1024), ("K", 1024),
            ("m", 1024 * 1024), ("M", 1024 * 1024),
            ("g", 1024 * 1024 * 1024), ("G", 1024 * 1024 * 1024),
        ];
        let (num_str, multiplier) = SUFFIXES.iter()
            .find(|(suf, _)| s.ends_with(suf))
            .map(|(suf, mult)| (&s[..s.len() - suf.len()], *mult))
            .unwrap_or((s, 1u64));
        let n = num_str.parse::<u64>()
            .map_err(|_| anyhow::anyhow!("invalid --max-filesize value: {s}"))?;
        Ok(Some(n * multiplier))
    }
}

pub fn parse() -> anyhow::Result<Args> {
    let mut args = Args::parse();

    // --include/--exclude are aliases for -g GLOB / -g !GLOB
    args.glob.extend(std::mem::take(&mut args.include));
    for g in std::mem::take(&mut args.exclude) {
        args.glob.push(format!("!{g}"));
    }

    // when -e/-f/-F is used or -p has words, positional PATTERN is a PATH
    let has_words = args.paragraphs.as_ref().map_or(false, |v| !v.is_empty())
        || args.fixed_strings.as_ref().map_or(false, |v| !v.is_empty());
    if (!args.regexp.is_empty() || !args.pattern_file.is_empty() || has_words || args.files)
        && args.pattern.is_some()
    {
        let pat = args.pattern.take().unwrap();
        args.paths.insert(0, PathBuf::from(pat));
    }

    // -a or -N without a base pattern: positional arg is a path, not a pattern
    let only_constraints = !args.and.is_empty()
        || (args.scope.as_deref() == Some("file") && !args.not.is_empty());
    if only_constraints
        && args.pattern.is_some()
        && args.regexp.is_empty()
        && args.pattern_file.is_empty()
        && !has_words
    {
        let pat = args.pattern.as_ref().unwrap();
        if std::path::Path::new(pat).exists() {
            let pat = args.pattern.take().unwrap();
            args.paths.insert(0, PathBuf::from(pat));
        }
    }

    // --scope paragraph is equivalent to -p (without words)
    if args.scope.as_deref() == Some("paragraph") && args.paragraphs.is_none() {
        args.paragraphs = Some(Vec::new());
    }

    // --scope file with only --not terms: invert to files-without-match
    let has_positive = args.pattern.is_some()
        || !args.regexp.is_empty()
        || !args.pattern_file.is_empty()
        || !args.and.is_empty()
        || args.paragraphs.as_ref().map_or(false, |v| !v.is_empty())
        || args.fixed_strings.as_ref().map_or(false, |v| !v.is_empty());
    if args.scope.as_deref() == Some("file") && !args.not.is_empty() && !has_positive {
        args.regexp = std::mem::take(&mut args.not);
        args.files_without_match = true;
    }

    // --scope file implies -l unless the user asked for specific output
    if args.scope.as_deref() == Some("file")
        && !args.count
        && !args.only_matching
        && !args.files_without_match
        && !args.quiet
        && !args.json
        && !args.count_matches
    {
        args.files_with_matches = true;
    }

    Ok(args)
}
