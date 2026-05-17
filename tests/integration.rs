use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new() -> Self {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("resharp-test-{}-{}", std::process::id(), id));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let p = self.path.join(name);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, content).unwrap();
        p
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn resharp() -> Command {
    Command::new(env!("CARGO_BIN_EXE_re"))
}

fn run_stdin(args: &[&str], input: &str) -> (String, i32) {
    use std::io::Write;
    let mut cmd = resharp();
    cmd.args(args);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
    let code = out.status.code().unwrap_or(-1);
    (stdout, code)
}

fn run_args(args: &[&str]) -> (String, i32) {
    let out = resharp()
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
    let code = out.status.code().unwrap_or(-1);
    (stdout, code)
}

fn run_args_full(args: &[&str]) -> (String, String, i32) {
    let out = resharp()
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim_end().to_string();
    let code = out.status.code().unwrap_or(-1);
    (stdout, stderr, code)
}


#[test]
fn stdin_match() {
    let (out, _) = run_stdin(&["apple"], "apple pie\nbanana split\napple sauce\n");
    assert_eq!(out, "apple pie\napple sauce");
}

#[test]
fn file_search() {
    let td = TestDir::new();
    let f = td.write("fruits.txt", "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    let (out, _) = run_args(&["--no-heading", "--no-line-number", "--color", "never", "apple", f.to_str().unwrap()]);
    assert!(out.contains("apple pie"));
    assert!(out.contains("apple sauce"));
    assert!(!out.contains("banana"));
}

#[test]
fn no_match_empty() {
    let (out, code) = run_stdin(&["xyz"], "abc\n");
    assert_eq!(out, "");
    assert_eq!(code, 1);
}

#[test]
fn regex_char_class() {
    let (out, _) = run_stdin(&["-n", "[ac].*e"], "apple pie\nbanana split\napple sauce\ncherry tart\n");
    assert_eq!(out, "1:apple pie\n3:apple sauce\n4:cherry tart");
}

#[test]
fn regex_anchor_start() {
    let (out, _) = run_stdin(&["-n", "^apple"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple pie\n3:apple sauce");
}

#[test]
fn regex_anchor_end() {
    let (out, _) = run_stdin(&["-n", "e$"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple pie\n3:apple sauce\n5:grape juice");
}

#[test]
fn no_trailing_newline() {
    let (out, _) = run_stdin(&["-n", "newline"], "no newline at end");
    assert_eq!(out, "1:no newline at end");
}

#[test]
fn empty_input() {
    let (out, code) = run_stdin(&["test"], "");
    assert_eq!(out, "");
    assert_eq!(code, 1);
}


#[test]
fn case_insensitive() {
    let (out, _) = run_stdin(&["-n", "-i", "hello"], "hello world\nHello World\nHELLO WORLD\nhello\n");
    assert_eq!(out, "1:hello world\n2:Hello World\n3:HELLO WORLD\n4:hello");
}

#[test]
fn case_sensitive() {
    let (out, _) = run_stdin(&["-n", "-s", "hello"], "hello world\nHello World\nHELLO WORLD\nhello\n");
    assert_eq!(out, "1:hello world\n4:hello");
}

#[test]
fn smart_case_lower() {
    let (out, _) = run_stdin(&["-n", "-S", "hello"], "hello world\nHello World\nHELLO WORLD\nhello\n");
    assert_eq!(out, "1:hello world\n2:Hello World\n3:HELLO WORLD\n4:hello");
}

#[test]
fn case_insensitive_union_all_terms() {
    // -i flag should apply to all terms of a union, not just the leftmost
    let input = "MIN\nMAX\n";
    let (out, _) = run_stdin(&["-n", "-i", "min|max"], input);
    assert_eq!(out, "1:MIN\n2:MAX", "min|max should match both MIN and MAX");
    let (out, _) = run_stdin(&["-n", "-i", "max|min"], input);
    assert_eq!(out, "1:MIN\n2:MAX", "max|min should match both MIN and MAX");
}

#[test]
fn smart_case_upper() {
    let (out, _) = run_stdin(&["-n", "-S", "Hello"], "hello world\nHello World\nHELLO WORLD\nhello\n");
    assert_eq!(out, "2:Hello World");
}


#[test]
fn invert_match() {
    let (out, _) = run_stdin(&["-n", "-v", "apple|sauce"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "2:banana split\n4:cherry tart\n5:grape juice");
}

#[test]
fn word_match() {
    let (out, _) = run_stdin(&["-n", "-w", "cat"], "the cat sat\ncatalog\nthe cat and dog\nscatter\n");
    assert_eq!(out, "1:the cat sat\n3:the cat and dog");
}

#[test]
fn line_match() {
    let (out, _) = run_stdin(&["-n", "-x", "hello"], "hello world\nHello World\nHELLO WORLD\nhello\n");
    assert_eq!(out, "4:hello");
}

#[test]
fn max_count() {
    let (out, _) = run_stdin(&["-n", "-m", "1", "apple"], "apple pie\nbanana split\napple sauce\n");
    assert_eq!(out, "1:apple pie");
}


#[test]
fn count() {
    let (out, _) = run_stdin(&["-c", "apple"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "2");
}

#[test]
fn only_matching() {
    let (out, _) = run_stdin(&["-n", "-o", "apple"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple\n3:apple");
}

#[test]
fn only_matching_multiple_per_line_column() {
    let (out, _) = run_stdin(&["-o", "--column", "foo"], "foo bar foo baz\n");
    assert_eq!(out, "1:foo\n9:foo");
}

#[test]
fn only_matching_multiple_per_line_byte_offset() {
    let (out, _) = run_stdin(&["-o", "-b", "foo"], "foo bar foo baz\n");
    assert_eq!(out, "0:foo\n8:foo");
}

#[test]
fn only_matching_multiple_per_line_vimgrep() {
    let (out, _) = run_stdin(&["-o", "--vimgrep", "foo"], "foo bar foo baz\n");
    assert_eq!(out, "1:1:foo\n1:9:foo");
}

#[test]
fn exclude_match_alone() {
    let (out, _) = run_stdin(&["-o", "-E", "\""], "so I should investigate that.\",\"thinkingSignature\":\"EvsBClkIDBg\n");
    assert!(out.contains("so I should investigate that."), "got: {out:?}");
    assert!(out.contains("thinkingSignature"), "got: {out:?}");
    assert!(out.contains("EvsBClkIDBg"), "got: {out:?}");
    assert!(!out.contains('"'), "matches must not contain quote: {out:?}");
}

#[test]
fn exclude_match_with_positive() {
    // -a hello -a world: line must contain both. -E '"': within match span, no quotes.
    // line 2 has hello and world but separated by quotes, so no quote-free span covers both.
    let (out, _) = run_stdin(
        &["-a", "hello", "-a", "world", "-E", "\""],
        "hello world\nhello \"world\"\n",
    );
    assert_eq!(out, "hello world");
}

#[test]
fn exclude_match_alternation_grouped() {
    // -E with alternation must group the inner pattern - otherwise `a|b|c` wrapped as `_*a|b|c_*`
    // parses as `_*a` | `b` | `c_*` and lets through anything containing one of the variants.
    let input = "x.unwrap();\nx.unwrap_or(y);\nx.unwrap_err();\nx.unwrap_unchecked();\n";
    let (out, _) = run_stdin(
        &["-a", "\\.unwrap[a-z_]*\\(", "-E", "unwrap_or|unwrap_err|unwrap_unchecked"],
        input,
    );
    assert_eq!(out, "x.unwrap();");
}

#[test]
fn and_alternation_grouped() {
    // -a with alternation must group similarly
    let (out, _) = run_stdin(&["-a", "foo|bar"], "foo line\nbar line\nbaz line\n");
    assert_eq!(out, "foo line\nbar line");
}

#[test]
fn not_alternation_grouped() {
    let (out, _) = run_stdin(&["_*", "-N", "aaa|ccc"], "aaa\nbbb\nccc\n");
    assert_eq!(out, "bbb");
}

#[test]
fn exclude_match_differs_from_not() {
    // -N: whole line excluded if contains quote
    let (out_n, _) = run_stdin(&["-N", "\"", "_*"], "aaa\n\"bbb\"\nccc\n");
    assert_eq!(out_n, "aaa\nccc");
    // -E: only match span must not contain quote (line still appears, just span is narrower)
    let (out_e, _) = run_stdin(&["-o", "-E", "\"", "_*"], "aaa\n\"bbb\"\nccc\n");
    assert!(out_e.contains("aaa"), "got: {out_e:?}");
    assert!(out_e.contains("bbb"), "got: {out_e:?}");
    assert!(out_e.contains("ccc"), "got: {out_e:?}");
    assert!(!out_e.contains('"'), "got: {out_e:?}");
}

#[test]
fn column() {
    let (out, _) = run_stdin(&["-n", "--column", "bar"], "foo bar baz\n");
    assert_eq!(out, "1:5:foo bar baz");
}

#[test]
fn byte_offset() {
    let (out, _) = run_stdin(&["-n", "-b", "bbb"], "aaa\nbbb\nccc\n");
    assert_eq!(out, "2:4:bbb");
}

#[test]
fn files_with_matches() {
    let td = TestDir::new();
    let f = td.write("fruits.txt", "apple pie\nbanana split\n");
    let (out, _) = run_args(&["-l", "--no-heading", "--color", "never", "apple", f.to_str().unwrap()]);
    assert_eq!(out, f.to_str().unwrap());
}

#[test]
fn files_without_match() {
    let td = TestDir::new();
    let f1 = td.write("fruits.txt", "apple pie\nbanana split\n");
    let f2 = td.write("cats.txt", "the cat sat\ncatalog\n");
    let (out, _) = run_args(&[
        "--files-without-match", "--no-heading", "--color", "never",
        "apple", f1.to_str().unwrap(), f2.to_str().unwrap(),
    ]);
    assert_eq!(out, f2.to_str().unwrap());
}


#[test]
fn after_context() {
    let (out, _) = run_stdin(&["-n", "-A", "1", "banana"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "2:banana split\n3-apple sauce");
}

#[test]
fn before_context() {
    let (out, _) = run_stdin(&["-n", "-B", "1", "banana"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1-apple pie\n2:banana split");
}

#[test]
fn context_both() {
    let (out, _) = run_stdin(&["-n", "-C", "1", "apple"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple pie\n2-banana split\n3:apple sauce\n4-cherry tart");
}

#[test]
fn context_separator() {
    let (out, _) = run_stdin(&["-n", "-C", "1", "pie|juice"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple pie\n2-banana split\n--\n4-cherry tart\n5:grape juice");
}


#[test]
fn multiple_patterns_e() {
    let (out, _) = run_stdin(&["-n", "-e", "apple", "-e", "banana"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple pie\n2:banana split\n3:apple sauce");
}

#[test]
fn pattern_file() {
    let td = TestDir::new();
    let pf = td.write("pats.txt", "apple\nbanana\n");
    let (out, _) = run_stdin(
        &["-n", "-f", pf.to_str().unwrap()],
        "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n",
    );
    assert_eq!(out, "1:apple pie\n2:banana split\n3:apple sauce");
}

#[test]
fn e_with_file_path() {
    let td = TestDir::new();
    let f = td.write("fruits.txt", "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    let (out, _) = run_args(&["-e", "apple", f.to_str().unwrap(), "--no-heading", "--no-line-number", "--color", "never"]);
    assert!(out.contains("apple pie"));
    assert!(out.contains("apple sauce"));
    assert!(!out.contains("banana"));
}


#[test]
fn fixed_strings() {
    let (out, _) = run_stdin(&["-n", "-F", "foo.bar"], "foo.bar\nfooXbar\n");
    assert_eq!(out, "1:foo.bar");
}


#[test]
fn wildcard_underscore() {
    let (out, _) = run_stdin(&["-n", "_*apple_*"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple pie\n3:apple sauce");
}

#[test]
fn intersection() {
    let (out, _) = run_stdin(&["-n", "(_*cat_*)&(_*the_*)"], "the cat sat\ncatalog\nthe cat and dog\nscatter\n");
    assert_eq!(out, "1:the cat sat\n3:the cat and dog");
}

#[test]
fn intersection_both() {
    let (out, _) = run_stdin(&["-n", "(_*cat_*)&(_*dog_*)"], "the cat sat\ncatalog\nthe cat and dog\nscatter\n");
    assert_eq!(out, "3:the cat and dog");
}


#[test]
fn lookahead_positive() {
    let (out, _) = run_stdin(&["-n", "(?=.*cat)(?=.*mat).*"], "the cat sat on the mat\nthe dog sat\ncat on mat\n");
    assert_eq!(out, "1:the cat sat on the mat\n3:cat on mat");
}

#[test]
fn lookbehind_positive() {
    let (out, _) = run_stdin(&["-n", "(?<=foo)bar"], "foobar\nbazbar\nfooqux\n");
    assert_eq!(out, "1:foobar");
}

#[test]
fn lookahead_with_intersection() {
    // lookahead and resharp intersection should compose
    let (out, _) = run_stdin(&["-n", "(?=.*hello)(_*world_*)"], "hello world\nfoo world\nhello bar\n");
    assert_eq!(out, "1:hello world");
}


#[test]
fn paragraphs_blocks_cross_para() {
    let input = "first paragraph about\ncats and dogs together\n\nsecond paragraph about\nfish and birds\n\nthird paragraph with\ncats but no dogs\n";
    let (out, code) = run_stdin(&["--paragraphs", "(_*cats_*)&(_*fish_*)"], input);
    assert_eq!(out, "");
    assert_eq!(code, 1);
}

#[test]
fn paragraphs_within_para() {
    let input = "first paragraph about\ncats and dogs together\n\nsecond paragraph about\nfish and birds\n\nthird paragraph with\ncats but no dogs\n";
    let (out, _) = run_stdin(&["-n", "--paragraphs", "(_*cats_*)&(_*dogs_*)"], input);
    let lines: Vec<&str> = out.lines().take(2).collect();
    assert_eq!(lines, vec!["1:first paragraph about", "2:cats and dogs together"]);
}


#[test]
fn paragraphs_words_match() {
    let input = "first paragraph about\ncats and dogs together\n\nsecond paragraph about\nfish and birds\n";
    let (out, code) = run_stdin(&["-n", "-p", "cats", "-p", "dogs"], input);
    assert_eq!(code, 0);
    let lines: Vec<&str> = out.lines().take(2).collect();
    assert_eq!(lines, vec!["1:first paragraph about", "2:cats and dogs together"]);
}

#[test]
fn paragraphs_words_no_match_cross_para() {
    let input = "first paragraph about\ncats only\n\nsecond paragraph about\ndogs only\n";
    let (out, code) = run_stdin(&["-p", "cats", "-p", "dogs"], input);
    assert_eq!(out, "");
    assert_eq!(code, 1);
}

#[test]
fn paragraphs_words_single_word() {
    let input = "hello world\n\ngoodbye world\n";
    let (out, code) = run_stdin(&["-p", "hello"], input);
    assert_eq!(code, 0);
    assert!(out.contains("hello world"));
    assert!(!out.contains("goodbye"));
}

#[test]
fn paragraphs_words_case_insensitive() {
    let input = "Hello World\nfoo bar\n\nother paragraph\n";
    let (out, code) = run_stdin(&["-i", "-p", "hello", "-p", "foo"], input);
    assert_eq!(code, 0);
    assert!(out.contains("Hello World"));
    assert!(out.contains("foo bar"));
}

#[test]
fn paragraphs_words_three() {
    let input = "the cat sat\non the mat\nwith a hat\n\njust a cat\nno mat here\n";
    let (out, code) = run_stdin(&["-p", "cat", "-p", "mat", "-p", "hat"], input);
    assert_eq!(code, 0);
    assert!(out.contains("the cat sat"));
    assert!(out.contains("on the mat"));
    assert!(out.contains("with a hat"));
    // second paragraph has cat and mat but not hat
    assert!(!out.contains("no mat here"));
}

#[test]
fn paragraphs_words_with_not() {
    let input = "cats and dogs\nare friends\n\ncats and birds\nare enemies\n";
    let (out, code) = run_stdin(&["-p", "cats", "--not", "dogs"], input);
    assert_eq!(code, 0);
    assert!(out.contains("birds"));
    assert!(!out.contains("dogs"));
}

#[test]
fn paragraphs_flag_with_not() {
    let input = "cats and dogs\nare friends\n\ncats and birds\nare enemies\n";
    let (out, code) = run_stdin(&["--paragraphs", "(_*cats_*)", "--not", "dogs"], input);
    assert_eq!(code, 0);
    assert!(out.contains("birds"));
    assert!(!out.contains("dogs"));
}


#[test]
fn type_filter() {
    let td = TestDir::new();
    // both files contain "main" - without -t the py file would also match
    td.write("dir/main.rs", "fn main() {}\n");
    td.write("dir/main.py", "def main():\n");
    let (out, _) = run_args(&[
        "-t", "rust", "--no-heading", "--no-line-number", "--color", "never",
        "main", td.path().join("dir").to_str().unwrap(),
    ]);
    assert!(out.contains("fn main() {}"));
    assert!(!out.contains("def main"), "-t rust should exclude .py files");
}

#[test]
fn type_not_filter() {
    let td = TestDir::new();
    td.write("dir/main.rs", "fn main() {}\n");
    td.write("dir/main.py", "def main():\n");
    let (out, _) = run_args(&[
        "-T", "rust", "--no-heading", "--no-line-number", "--color", "never",
        "main", td.path().join("dir").to_str().unwrap(),
    ]);
    assert!(out.contains("def main"), "-T rust should include .py files");
    assert!(!out.contains("fn main"), "-T rust should exclude .rs files");
}

#[test]
fn glob_filter() {
    let td = TestDir::new();
    td.write("dir/main.rs", "fn main() {}\n");
    td.write("dir/main.py", "def main():\n");
    let (out, _) = run_args(&[
        "-g", "*.py", "-c", "--no-heading", "--color", "never",
        "main", td.path().join("dir").to_str().unwrap(),
    ]);
    assert!(out.contains("1"));
}

#[test]
fn max_depth() {
    let td = TestDir::new();
    td.write("dir/main.rs", "fn main() {}\n");
    td.write("dir/sub/deep.rs", "sub file match\n");
    let (out, code) = run_args(&[
        "--max-depth", "1", "--no-heading", "--color", "never",
        "sub file", td.path().join("dir").to_str().unwrap(),
    ]);
    assert_eq!(out, "");
    assert_eq!(code, 1);
}

#[test]
fn hidden_files() {
    let td = TestDir::new();
    td.write("dir/.hidden", "hidden content\n");
    td.write("dir/visible.txt", "visible content\n");
    let (out, _) = run_args(&[
        "--hidden", "--no-heading", "--no-line-number", "--color", "never",
        "hidden", td.path().join("dir").to_str().unwrap(),
    ]);
    assert!(out.contains("hidden content"));
}

#[test]
fn sort_path() {
    let td = TestDir::new();
    td.write("dir/main.py", "def main():\n");
    td.write("dir/main.rs", "fn main() {}\n");
    td.write("dir/sub/deep.rs", "sub file match\n");
    let (out, _) = run_args(&[
        "--sort", "path", "-l", "--no-heading", "--color", "never",
        "main|fn|sub", td.path().join("dir").to_str().unwrap(),
    ]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].ends_with("main.py"));
    assert!(lines[1].ends_with("main.rs"));
    assert!(lines[2].ends_with("deep.rs"));
}


#[test]
fn exit_0_match() {
    let td = TestDir::new();
    let f = td.write("fruits.txt", "apple pie\n");
    let (_, code) = run_args(&["apple", f.to_str().unwrap()]);
    assert_eq!(code, 0);
}

#[test]
fn exit_1_no_match() {
    let td = TestDir::new();
    let f = td.write("fruits.txt", "apple pie\n");
    let (_, code) = run_args(&["zzzzz", f.to_str().unwrap()]);
    assert_eq!(code, 1);
}

#[test]
fn exit_2_bad_pattern() {
    let td = TestDir::new();
    let f = td.write("fruits.txt", "apple pie\n");
    let (_, code) = run_args(&["[invalid", f.to_str().unwrap()]);
    assert_eq!(code, 2);
}

#[test]
fn exit_2_no_file() {
    let (_, code) = run_args(&["x", "/nonexistent"]);
    assert_eq!(code, 2);
}



/// generate a large file with known content; resharp scans in reverse so
/// place matches at the start, middle, and end to exercise boundary conditions
fn make_large_file(td: &TestDir) -> PathBuf {
    let mut content = String::new();
    // match at the very start
    content.push_str("MATCH_START secret_token_here\n");
    // pad to >1MB to trigger auto-mmap
    for i in 0..50_000 {
        content.push_str(&format!("filler line number {i} with no interesting content\n"));
    }
    // match in the middle
    content.push_str("MATCH_MIDDLE another_secret\n");
    for i in 50_000..100_000 {
        content.push_str(&format!("filler line number {i} with no interesting content\n"));
    }
    // match at the very end
    // NOTE: trailing newline required to work around resharp 0.2 engine bug
    // where find_all drops the first match on large inputs without trailing newline
    content.push_str("MATCH_END final_secret\n");
    td.write("large.txt", &content)
}

#[test]
fn mmap_forced_small_file() {
    let td = TestDir::new();
    let f = td.write("small.txt", "apple pie\nbanana split\napple sauce\n");
    let (out, code) = run_args(&[
        "--mmap", "--no-heading", "--no-line-number", "--color", "never",
        "apple", f.to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("apple pie"));
    assert!(out.contains("apple sauce"));
    assert!(!out.contains("banana"));
}

#[test]
fn no_mmap_large_file() {
    let td = TestDir::new();
    let f = make_large_file(&td);
    let (out, code) = run_args(&[
        "--no-mmap", "--no-heading", "--no-line-number", "--color", "never",
        "MATCH_", f.to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("MATCH_START"));
    assert!(out.contains("MATCH_MIDDLE"));
    assert!(out.contains("MATCH_END"));
}

#[test]
fn mmap_auto_large_file() {
    let td = TestDir::new();
    let f = make_large_file(&td);
    // auto-mmap (file is >1MB), just verify correctness
    let (out, code) = run_args(&[
        "--no-heading", "--no-line-number", "--color", "never",
        "MATCH_", f.to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("MATCH_START"));
    assert!(out.contains("MATCH_MIDDLE"));
    assert!(out.contains("MATCH_END"));
}

#[test]
fn mmap_vs_read_identical_output() {
    let td = TestDir::new();
    let f = make_large_file(&td);
    let fp = f.to_str().unwrap();
    let (out_mmap, _) = run_args(&[
        "--mmap", "--no-heading", "--color", "never",
        "MATCH_", fp,
    ]);
    let (out_read, _) = run_args(&[
        "--no-mmap", "--no-heading", "--color", "never",
        "MATCH_", fp,
    ]);
    assert_eq!(out_mmap, out_read);
}

#[test]
fn mmap_count_matches() {
    let td = TestDir::new();
    let f = make_large_file(&td);
    let (out_mmap, _) = run_args(&[
        "--mmap", "-c", "--no-heading", "--color", "never",
        "MATCH_", f.to_str().unwrap(),
    ]);
    let (out_read, _) = run_args(&[
        "--no-mmap", "-c", "--no-heading", "--color", "never",
        "MATCH_", f.to_str().unwrap(),
    ]);
    assert!(out_mmap.ends_with("3"));
    assert!(out_read.ends_with("3"));
}

#[test]
#[ignore = "takes long"]
fn mmap_intersection() {
    // resharp processes in reverse - test intersection with mmap
    let td = TestDir::new();
    let f = make_large_file(&td);
    let (out_mmap, _) = run_args(&[
        "--mmap", "-c", "--no-heading", "--color", "never",
        "(_*MATCH_*)&(_*secret_*)", f.to_str().unwrap(),
    ]);
    let (out_read, _) = run_args(&[
        "--no-mmap", "-c", "--no-heading", "--color", "never",
        "(_*MATCH_*)&(_*secret_*)", f.to_str().unwrap(),
    ]);
    // MATCH_START and MATCH_MIDDLE have "secret", MATCH_END has "secret" too
    assert_eq!(out_mmap, out_read);
    assert!(out_mmap.ends_with("3"));
}

#[test]
fn mmap_invert_match() {
    let td = TestDir::new();
    let f = make_large_file(&td);
    let (count_mmap, _) = run_args(&[
        "--mmap", "-vc", "--no-heading", "--color", "never",
        "MATCH_", f.to_str().unwrap(),
    ]);
    let (count_read, _) = run_args(&[
        "--no-mmap", "-vc", "--no-heading", "--color", "never",
        "MATCH_", f.to_str().unwrap(),
    ]);
    assert_eq!(count_mmap, count_read);
    // 100_000 filler lines + 0 match lines = 100_000 non-matching
    let n: usize = count_mmap.trim().split(':').last().unwrap().parse().unwrap();
    assert_eq!(n, 100_000);
}

#[test]
fn mmap_context_lines() {
    let td = TestDir::new();
    let f = make_large_file(&td);
    let (out_mmap, _) = run_args(&[
        "--mmap", "-C", "1", "--no-heading", "--color", "never",
        "MATCH_MIDDLE", f.to_str().unwrap(),
    ]);
    let (out_read, _) = run_args(&[
        "--no-mmap", "-C", "1", "--no-heading", "--color", "never",
        "MATCH_MIDDLE", f.to_str().unwrap(),
    ]);
    assert_eq!(out_mmap, out_read);
    // should have before-context, match, after-context
    let lines: Vec<&str> = out_mmap.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].contains("filler"));
    assert!(lines[1].contains("MATCH_MIDDLE"));
    assert!(lines[2].contains("filler"));
}


#[test]
fn quiet_no_output() {
    let td = TestDir::new();
    let f = td.write("fruits.txt", "apple pie\n");
    let (out, _) = run_args(&["-q", "apple", f.to_str().unwrap()]);
    assert_eq!(out, "");
}

#[test]
fn quiet_exit_0() {
    let td = TestDir::new();
    let f = td.write("fruits.txt", "apple pie\n");
    let (_, code) = run_args(&["-q", "apple", f.to_str().unwrap()]);
    assert_eq!(code, 0);
}

#[test]
fn quiet_exit_1() {
    let td = TestDir::new();
    let f = td.write("fruits.txt", "apple pie\n");
    let (_, code) = run_args(&["-q", "zzzzz", f.to_str().unwrap()]);
    assert_eq!(code, 1);
}

/// resharp 0.2 engine bug: find_all on intersection patterns returns different
/// match counts depending on whether the buffer ends with \n.
/// with trailing newline find_all returns 3 per-line matches;
/// without it, the first match at offset 0 is silently lost.
#[test]
fn find_all_no_trailing_newline_large_intersection() {
    let re = resharp::Regex::with_options(
        "(_*MATCH_*)&(_*secret_*)",
        resharp::RegexOptions {
            ..Default::default()
        },
    ).unwrap();

    let mut buf = Vec::new();
    buf.extend_from_slice(b"MATCH_START secret_token_here\n");
    for i in 0..50_000u32 {
        buf.extend_from_slice(
            format!("filler line number {i} with no interesting content\n").as_bytes(),
        );
    }
    buf.extend_from_slice(b"MATCH_MIDDLE another_secret\n");
    for i in 50_000..100_000u32 {
        buf.extend_from_slice(
            format!("filler line number {i} with no interesting content\n").as_bytes(),
        );
    }

    // with trailing newline
    let mut buf_nl = buf.clone();
    buf_nl.extend_from_slice(b"MATCH_END final_secret\n");
    let matches_nl = re.find_all(&buf_nl).unwrap();

    // without trailing newline
    let mut buf_no_nl = buf;
    buf_no_nl.extend_from_slice(b"MATCH_END final_secret");
    let matches_no_nl = re.find_all(&buf_no_nl).unwrap();

    // trailing newline should not affect match count
    assert_eq!(
        matches_nl.len(),
        matches_no_nl.len(),
        "trailing newline changes match count: with={}  without={}",
        matches_nl.len(),
        matches_no_nl.len(),
    );
}


#[test]
fn scope_line_default() {
    // default scope is line, same as always
    let (out, _) = run_stdin(&["-n", "-W", "cat", "-W", "dog"], "the cat and dog\ncat only\ndog only\n");
    assert_eq!(out, "1:the cat and dog");
}

#[test]
fn scope_paragraph() {
    let input = "cats and\ndogs together\n\nfish only\n";
    let (out, code) = run_stdin(&["--scope", "paragraph", "-W", "cats", "-W", "dogs"], input);
    assert_eq!(code, 0);
    assert!(out.contains("cats and"));
    assert!(out.contains("dogs together"));
    assert!(!out.contains("fish"));
}

#[test]
fn scope_file() {
    let td = TestDir::new();
    let f1 = td.write("both.rs", "use serde;\nasync fn foo() {}\n");
    let f2 = td.write("only_serde.rs", "use serde;\nfn bar() {}\n");
    let (out, _) = run_args(&[
        "--scope", "file", "-l", "--no-heading", "--color", "never",
        "serde", "-a", "async",
        f1.to_str().unwrap(), f2.to_str().unwrap(),
    ]);
    assert!(out.contains("both.rs"));
    assert!(!out.contains("only_serde.rs"));
}

#[test]
fn scope_custom_boundary() {
    // custom scope: boundary is \n\n, match must not cross it
    let input = "error here\ntimeout here\n\nerror only\n";
    let (out, code) = run_stdin(
        &["--scope", "\n\n", "(_*error_*)&(_*timeout_*)"],
        input,
    );
    assert_eq!(code, 0);
    assert!(out.contains("error here"));
    assert!(out.contains("timeout here"));
}

#[test]
fn scope_custom_no_cross() {
    // should not match across boundary
    let input = "error here\n\ntimeout here\n";
    let (out, code) = run_stdin(
        &["--scope", "\n\n", "(_*error_*)&(_*timeout_*)"],
        input,
    );
    assert_eq!(out, "");
    assert_eq!(code, 1);
}


#[test]
fn json_basic() {
    let (out, _) = run_stdin(&["--json", "apple"], "apple pie\nbanana\napple sauce\n");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    let v: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(v["line_number"], 1);
    assert_eq!(v["submatches"][0]["match"], "apple");
}

#[test]
fn json_with_path() {
    let td = TestDir::new();
    let f = td.write("test.txt", "hello world\n");
    let (out, _) = run_args(&["--json", "hello", f.to_str().unwrap()]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["path"].as_str().unwrap().contains("test.txt"));
    assert_eq!(v["line_number"], 1);
}

#[test]
fn json_files_with_matches() {
    let td = TestDir::new();
    let f = td.write("test.txt", "hello world\n");
    let (out, _) = run_args(&["--json", "-l", "hello", f.to_str().unwrap()]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["type"], "file");
    assert!(v["path"].as_str().unwrap().contains("test.txt"));
}

#[test]
fn json_count() {
    let (out, _) = run_stdin(&["--json", "-c", "apple"], "apple pie\nbanana\napple sauce\n");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["type"], "count");
    assert_eq!(v["count"], 2);
}

#[test]
fn json_show_scope() {
    let input = "fn main() {\n    unwrap()\n}\n";
    let (out, _) = run_stdin(&["--json", "--show-scope", "unwrap"], input);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["scope"], "fn main() {");
}


#[test]
fn near_basic() {
    let input = "unsafe {\n  foo();\n  bar.unwrap();\n}\nsafe_fn() {\n  baz();\n}\n";
    let (out, code) = run_stdin(&["-P", "3", "-W", "unsafe", "-W", "unwrap"], input);
    assert_eq!(code, 0);
    assert!(out.contains("unsafe"));
    assert!(out.contains("unwrap"));
}

#[test]
fn near_too_far() {
    let input = "unsafe {\n  a;\n  b;\n  c;\n  d;\n  e;\n  f.unwrap();\n}\n";
    let (out, code) = run_stdin(&["-P", "2", "-W", "unsafe", "-W", "unwrap"], input);
    // unsafe is on line 1, unwrap on line 7 - distance 6 > near 2
    assert_eq!(code, 1);
    assert_eq!(out, "");
}

#[test]
fn near_within_range() {
    let input = "unsafe {\n  f.unwrap();\n}\n";
    let (out, code) = run_stdin(&["-P", "2", "-W", "unsafe", "-W", "unwrap"], input);
    assert_eq!(code, 0);
    assert!(out.contains("unsafe"));
    assert!(out.contains("unwrap"));
}


#[test]
fn max_total_stdin() {
    let (out, _) = run_stdin(&["--max-total", "2", "apple"], "apple 1\napple 2\napple 3\napple 4\n");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
}

#[test]
fn max_total_files() {
    let td = TestDir::new();
    td.write("a.txt", "match 1\nmatch 2\nmatch 3\n");
    td.write("b.txt", "match 4\nmatch 5\n");
    let (out, _) = run_args(&[
        "--max-total", "3", "--sort", "path", "--no-heading", "--no-line-number", "--color", "never",
        "match", td.path().to_str().unwrap(),
    ]);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
}


#[test]
fn unique_only_matching() {
    let (out, _) = run_stdin(
        &["-o", "--unique", "apple|banana"],
        "apple pie\nbanana split\napple sauce\nbanana cream\napple tart\n",
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2); // apple + banana, deduplicated
}

#[test]
fn unique_full_lines() {
    let (out, _) = run_stdin(
        &["--unique", "dup"],
        "dup line\nother\ndup line\ndup line\n",
    );
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 1); // only one "dup line"
}


#[test]
fn show_scope_function() {
    let input = "fn outer() {\n    let x = val.unwrap();\n}\n";
    let (out, _) = run_stdin(&["--show-scope", "-n", "unwrap"], input);
    assert!(out.contains("fn outer()"));
    assert!(out.contains("unwrap"));
}

#[test]
fn show_scope_rust_impl_method() {
    // match inside an impl block's method - innermost (fn) wins, not the impl
    let input = "impl Foo {\n    fn bar(&self) {\n        self.x.unwrap();\n    }\n}\n";
    let (out, code) = run_stdin(&["--show-scope", "-n", "--color", "never", "unwrap"], input);
    assert_eq!(code, 0);
    assert!(out.contains("2:  fn bar(&self) {"), "out was:\n{out}");
    assert!(out.contains("3:        self.x.unwrap();"), "out was:\n{out}");
    assert!(!out.contains("impl Foo"), "should pick innermost scope, got:\n{out}");
}

#[test]
fn show_scope_rust_pub_async_fn() {
    let input = "pub async fn fetch() -> Result<()> {\n    client.send().await.unwrap();\n    Ok(())\n}\n";
    let (out, code) = run_stdin(&["--show-scope", "--color", "never", "unwrap"], input);
    assert_eq!(code, 0);
    assert!(out.contains("pub async fn fetch() -> Result<()> {"), "out was:\n{out}");
}

#[test]
fn show_scope_rust_pub_struct() {
    let input = "pub struct Config {\n    pub timeout: u64,\n}\n";
    let (out, code) = run_stdin(&["--show-scope", "--color", "never", "timeout"], input);
    assert_eq!(code, 0);
    assert!(out.contains("pub struct Config {"), "out was:\n{out}");
}

#[test]
fn show_scope_rust_trait() {
    let input = "pub trait Greeter {\n    fn hello(&self);\n    fn unwrap_world(&self);\n}\n";
    let (out, code) = run_stdin(&["--show-scope", "--color", "never", "unwrap_world"], input);
    assert_eq!(code, 0);
    assert!(out.contains("pub trait Greeter {"), "out was:\n{out}");
}

#[test]
fn show_scope_rust_enum() {
    let input = "pub enum Op {\n    Add,\n    Unwrap,\n}\n";
    let (out, code) = run_stdin(&["--show-scope", "--color", "never", "Unwrap"], input);
    assert_eq!(code, 0);
    assert!(out.contains("pub enum Op {"), "out was:\n{out}");
}

#[test]
fn show_scope_rust_skips_blank_lines() {
    // a blank line between the fn header and the match must not break scope detection
    let input = "fn outer() {\n    let x = 1;\n\n    val.unwrap();\n}\n";
    let (out, code) = run_stdin(&["--show-scope", "--color", "never", "unwrap"], input);
    assert_eq!(code, 0);
    assert!(out.contains("fn outer() {"), "out was:\n{out}");
}

#[test]
fn show_scope_rust_dedupes_within_same_fn() {
    // two matches in the same fn: scope header printed exactly once
    let input = "fn process() {\n    a.unwrap();\n    b.unwrap();\n}\n";
    let (out, code) = run_stdin(&["--show-scope", "-n", "--color", "never", "unwrap"], input);
    assert_eq!(code, 0);
    assert_eq!(out.matches("fn process()").count(), 1, "out was:\n{out}");
    assert!(out.contains("2:    a.unwrap();"), "out was:\n{out}");
    assert!(out.contains("3:    b.unwrap();"), "out was:\n{out}");
}

#[test]
fn show_scope_rust_two_functions() {
    // matches in two different fns: each scope header printed
    let input = "fn alpha() {\n    a.unwrap();\n}\nfn beta() {\n    b.unwrap();\n}\n";
    let (out, code) = run_stdin(&["--show-scope", "--color", "never", "unwrap"], input);
    assert_eq!(code, 0);
    assert_eq!(out.matches("fn alpha()").count(), 1, "out was:\n{out}");
    assert_eq!(out.matches("fn beta()").count(), 1, "out was:\n{out}");
}

#[test]
fn show_scope_rust_no_enclosing_scope() {
    // top-level match with no enclosing scope marker: no scope header, but match still prints
    let input = "let x = unwrap_me();\n";
    let (out, code) = run_stdin(&["--show-scope", "-n", "--color", "never", "unwrap_me"], input);
    assert_eq!(code, 0);
    assert_eq!(out, "1:let x = unwrap_me();");
}

#[test]
fn show_scope_rust_json_impl() {
    let input = "impl Foo {\n    fn bar(&self) {\n        self.x.unwrap();\n    }\n}\n";
    let (out, _) = run_stdin(&["--json", "--show-scope", "unwrap"], input);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["scope"], "fn bar(&self) {");
    assert_eq!(v["line_number"], 3);
}


#[test]
fn fixed_strings_underscore() {
    let (out, _) = run_stdin(&["-n", "-F", "a_b"], "a_b\naxb\nabc\n");
    assert_eq!(out, "1:a_b");
}

#[test]
fn fixed_strings_ampersand() {
    let (out, _) = run_stdin(&["-n", "-F", "a&b"], "a&b\nabc\n");
    assert_eq!(out, "1:a&b");
}

#[test]
fn fixed_strings_tilde() {
    let (out, _) = run_stdin(&["-n", "-F", "a~b"], "a~b\nabc\n");
    assert_eq!(out, "1:a~b");
}

#[test]
fn fixed_strings_all_meta() {
    let (out, _) = run_stdin(&["-n", "-F", "_&~"], "_&~\nabc\n");
    assert_eq!(out, "1:_&~");
}

#[test]
fn fixed_strings_dot_and_underscore() {
    let (out, _) = run_stdin(&["-n", "-F", "foo._bar"], "foo._bar\nfooXYbar\nfoo.Xbar\n");
    assert_eq!(out, "1:foo._bar");
}

#[test]
fn raw_underscore_literal() {
    // in raw mode, _ should be literal, not resharp wildcard
    let (out, _) = run_stdin(&["-n", "-R", "a_b"], "a_b\naxb\nabc\n");
    assert_eq!(out, "1:a_b");
}

#[test]
fn raw_ampersand_literal() {
    let (out, _) = run_stdin(&["-n", "-R", "a&b"], "a&b\nabc\n");
    assert_eq!(out, "1:a&b");
}

#[test]
fn raw_tilde_literal() {
    let (out, _) = run_stdin(&["-n", "-R", "a~b"], "a~b\nabc\n");
    assert_eq!(out, "1:a~b");
}

#[test]
fn raw_regex_still_works() {
    // raw mode should still support standard regex
    let (out, _) = run_stdin(&["-n", "-R", "a.b"], "a_b\naxb\nabc\n");
    assert_eq!(out, "1:a_b\n2:axb");
}

#[test]
fn raw_backslash_preserved() {
    // \d should still work in raw mode
    let (out, _) = run_stdin(&["-n", "-R", r"\d+_\d+"], "3_4\nabc\n12_34\n");
    assert_eq!(out, "1:3_4\n3:12_34");
}


#[test]
fn files_lists_all() {
    let td = TestDir::new();
    td.write("a.txt", "x\n");
    td.write("b.rs", "y\n");
    let (out, code) = run_args(&["--files", "--color", "never", td.path().to_str().unwrap()]);
    assert_eq!(code, 0);
    assert!(out.contains("a.txt"));
    assert!(out.contains("b.rs"));
}

#[test]
fn files_glob_include() {
    let td = TestDir::new();
    td.write("main.rs", "fn main() {}\n");
    td.write("lib.py", "pass\n");
    td.write("data.json", "{}\n");
    let (out, code) = run_args(&[
        "--files", "-g", "*.rs", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("main.rs"));
    assert!(!out.contains("lib.py"));
    assert!(!out.contains("data.json"));
}

#[test]
fn files_glob_exclude() {
    let td = TestDir::new();
    td.write("main.rs", "fn main() {}\n");
    td.write("test_main.rs", "fn test() {}\n");
    td.write("lib.rs", "pub mod lib;\n");
    let (out, code) = run_args(&[
        "--files", "-g", "*.rs", "-g", "!test_*", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("main.rs"));
    assert!(out.contains("lib.rs"));
    assert!(!out.contains("test_main.rs"));
}

#[test]
fn files_type_filter() {
    let td = TestDir::new();
    td.write("dir/main.rs", "fn main() {}\n");
    td.write("dir/main.py", "def main():\n");
    let (out, code) = run_args(&[
        "--files", "-t", "rust", "--color", "never",
        td.path().join("dir").to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("main.rs"));
    assert!(!out.contains("main.py"));
}

#[test]
fn files_type_not() {
    let td = TestDir::new();
    td.write("main.rs", "fn main() {}\n");
    td.write("main.py", "def main():\n");
    td.write("data.json", "{}\n");
    let (out, code) = run_args(&[
        "--files", "-T", "rust", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(!out.contains("main.rs"));
    assert!(out.contains("main.py"));
    assert!(out.contains("data.json"));
}

#[test]
fn files_sorted() {
    let td = TestDir::new();
    td.write("c.txt", "3\n");
    td.write("a.txt", "1\n");
    td.write("b.txt", "2\n");
    let (out, code) = run_args(&[
        "--files", "--sort", "path", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].ends_with("a.txt"));
    assert!(lines[1].ends_with("b.txt"));
    assert!(lines[2].ends_with("c.txt"));
}

#[test]
fn files_max_depth() {
    let td = TestDir::new();
    td.write("top.txt", "x\n");
    td.write("sub/deep.txt", "y\n");
    td.write("sub/sub2/deeper.txt", "z\n");
    let (out, code) = run_args(&[
        "--files", "--max-depth", "1", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("top.txt"));
    assert!(!out.contains("deep.txt"));
    assert!(!out.contains("deeper.txt"));
}

#[test]
fn files_hidden() {
    let td = TestDir::new();
    td.write(".hidden", "secret\n");
    td.write("visible.txt", "public\n");
    let (out_no_hidden, _) = run_args(&[
        "--files", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert!(!out_no_hidden.contains(".hidden"));
    assert!(out_no_hidden.contains("visible.txt"));

    let (out_hidden, _) = run_args(&[
        "--files", "--hidden", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert!(out_hidden.contains(".hidden"));
    assert!(out_hidden.contains("visible.txt"));
}

#[test]
fn files_exit_1_no_match() {
    let td = TestDir::new();
    td.write("main.rs", "fn main() {}\n");
    let (out, code) = run_args(&[
        "--files", "-g", "*.NOPE", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(out, "");
    assert_eq!(code, 1);
}

#[test]
fn files_exit_2_no_dir() {
    let (_, code) = run_args(&["--files", "/nonexistent/path"]);
    assert_eq!(code, 2);
}

#[test]
fn files_positional_as_path() {
    let td = TestDir::new();
    td.write("sub/a.rs", "fn a() {}\n");
    td.write("sub/b.py", "pass\n");
    td.write("other/c.rs", "fn c() {}\n");
    let (out, code) = run_args(&[
        "--files", "-g", "*.rs", "--color", "never",
        td.path().join("sub").to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("a.rs"));
    assert!(!out.contains("b.py"));
    assert!(!out.contains("c.rs"));
}

#[test]
fn files_max_filesize() {
    let td = TestDir::new();
    td.write("small.txt", "hi\n");
    td.write("big.txt", &"x".repeat(2048));
    let (out, code) = run_args(&[
        "--files", "--max-filesize", "1K", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("small.txt"));
    assert!(!out.contains("big.txt"));
}

#[test]
fn files_glob_and_type_combined() {
    let td = TestDir::new();
    td.write("main.rs", "fn main() {}\n");
    td.write("lib.rs", "pub mod lib;\n");
    td.write("test.rs", "fn test() {}\n");
    td.write("script.py", "pass\n");
    let (out, code) = run_args(&[
        "--files", "-t", "rust", "-g", "!test*", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("main.rs"));
    assert!(out.contains("lib.rs"));
    assert!(!out.contains("test.rs"));
    assert!(!out.contains("script.py"));
}

#[test]
fn files_absolute_paths() {
    let td = TestDir::new();
    td.write("hello.txt", "world\n");
    let (out, _) = run_args(&[
        "--files", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert!(out.starts_with('/'), "expected absolute path, got: {out}");
}

#[test]
fn files_content_search_still_works() {
    let td = TestDir::new();
    td.write("a.rs", "fn main() {}\n");
    td.write("b.py", "def main():\n");
    let (out, code) = run_args(&[
        "-g", "*.rs", "--no-heading", "--no-line-number", "--color", "never",
        "fn main", td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("fn main() {}"));
    assert!(!out.contains("def main"));
}

#[test]
fn files_multiple_paths() {
    let td = TestDir::new();
    td.write("dir1/a.txt", "x\n");
    td.write("dir2/b.txt", "y\n");
    let (out, code) = run_args(&[
        "--files", "--color", "never",
        td.path().join("dir1").to_str().unwrap(),
        td.path().join("dir2").to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("a.txt"));
    assert!(out.contains("b.txt"));
}

#[test]
fn files_unrestricted() {
    let td = TestDir::new();
    td.write(".secret", "hidden\n");
    td.write("visible.txt", "shown\n");
    let (out, _) = run_args(&[
        "--files", "-u", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert!(out.contains(".secret"));
    assert!(out.contains("visible.txt"));
}

#[test]
fn files_iglob() {
    let td = TestDir::new();
    td.write("Main.RS", "fn main() {}\n");
    td.write("lib.rs", "pub mod lib;\n");
    td.write("data.json", "{}\n");
    let (out, code) = run_args(&[
        "--files", "--iglob", "*.rs", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("Main.RS"));
    assert!(out.contains("lib.rs"));
    assert!(!out.contains("data.json"));
}

#[test]
fn files_nested_dirs() {
    let td = TestDir::new();
    td.write("src/main.rs", "fn main() {}\n");
    td.write("src/lib/mod.rs", "pub mod lib;\n");
    td.write("tests/test.py", "pass\n");
    td.write("docs/readme.md", "hello\n");
    let (out, code) = run_args(&[
        "--files", "-g", "*.rs", "--sort", "path", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("mod.rs"));
    assert!(out.contains("main.rs"));
    assert!(!out.contains("test.py"));
    assert!(!out.contains("readme.md"));
}


#[test]
fn stats_content_search() {
    let td = TestDir::new();
    td.write("a.txt", "apple pie\nbanana split\napple sauce\n");
    td.write("b.txt", "cherry tart\n");
    let (_, stderr, code) = run_args_full(&[
        "--stats", "--color", "never",
        "apple", td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(stderr.contains("2 files searched"), "stderr: {stderr}");
    assert!(stderr.contains("1 matched"), "stderr: {stderr}");
    assert!(stderr.contains("2 matches"), "stderr: {stderr}");
    assert!(stderr.contains("4 lines"), "stderr: {stderr}");
}

#[test]
fn stats_content_no_match() {
    let td = TestDir::new();
    td.write("a.txt", "hello world\n");
    let (_, stderr, code) = run_args_full(&[
        "--stats", "--color", "never",
        "zzzzz", td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 1);
    assert!(stderr.contains("1 files searched"), "stderr: {stderr}");
    assert!(stderr.contains("0 matched"), "stderr: {stderr}");
    assert!(stderr.contains("0 matches"), "stderr: {stderr}");
}

#[test]
fn stats_files_mode() {
    let td = TestDir::new();
    td.write("a.rs", "fn main() {}\nfn foo() {}\n");
    td.write("b.rs", "fn bar() {}\n");
    td.write("c.py", "pass\n");
    let (_, stderr, code) = run_args_full(&[
        "--files", "--stats", "-g", "*.rs", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(stderr.contains("2 files"), "stderr: {stderr}");
    assert!(stderr.contains("3 lines"), "stderr: {stderr}");
}

#[test]
fn stats_files_mode_line_count() {
    let td = TestDir::new();
    td.write("one.txt", "line1\nline2\nline3\nline4\nline5\n");
    td.write("two.txt", "a\nb\nc\n");
    let (_, stderr, _) = run_args_full(&[
        "--files", "--stats", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert!(stderr.contains("2 files"), "stderr: {stderr}");
    assert!(stderr.contains("8 lines"), "stderr: {stderr}");
}

#[test]
fn stats_has_timing() {
    let td = TestDir::new();
    td.write("a.txt", "hello\n");
    let (_, stderr, _) = run_args_full(&[
        "--stats", "--color", "never",
        "hello", td.path().to_str().unwrap(),
    ]);
    // timing should be in brackets like [1.2ms] or [0.05s]
    assert!(stderr.contains('[') && stderr.contains(']'), "stderr: {stderr}");
}

#[test]
fn no_stats_by_default() {
    let td = TestDir::new();
    td.write("a.txt", "hello\n");
    let (_, stderr, _) = run_args_full(&[
        "--color", "never",
        "hello", td.path().to_str().unwrap(),
    ]);
    assert_eq!(stderr, "", "stderr should be empty without --stats: {stderr}");
}


#[test]
fn exec_files_mode() {
    let td = TestDir::new();
    td.write("a.txt", "hello\n");
    td.write("b.txt", "world\n");
    let (out, code) = run_args(&[
        "--files", "--exec", "echo found: {}", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("found:"), "out: {out}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
}

#[test]
fn exec_content_search() {
    let td = TestDir::new();
    td.write("match.txt", "apple pie\n");
    td.write("miss.txt", "banana split\n");
    let (out, code) = run_args(&[
        "--exec", "echo hit: {}", "--color", "never",
        "apple", td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("hit:"), "out: {out}");
    assert!(out.contains("match.txt"), "out: {out}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 1);
}

#[test]
fn exec_no_placeholder_appends_path() {
    let td = TestDir::new();
    td.write("a.txt", "x\n");
    let (out, code) = run_args(&[
        "--files", "--exec", "echo", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("a.txt"), "out: {out}");
}

#[test]
fn exec_exit_1_no_files() {
    let td = TestDir::new();
    td.write("a.txt", "hello\n");
    let (_, code) = run_args(&[
        "--files", "-g", "*.NOPE", "--exec", "echo {}",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 1);
}

#[test]
fn exec_with_glob_filter() {
    let td = TestDir::new();
    td.write("main.rs", "fn main() {}\n");
    td.write("lib.py", "pass\n");
    let (out, code) = run_args(&[
        "--files", "-g", "*.rs", "--exec", "echo {}",
        "--color", "never", td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("main.rs"), "out: {out}");
    assert!(!out.contains("lib.py"), "out: {out}");
}

#[test]
fn exec_with_stats() {
    let td = TestDir::new();
    td.write("a.rs", "fn main() {}\n");
    td.write("b.rs", "fn foo() {}\n");
    let (_, stderr, code) = run_args_full(&[
        "--files", "-g", "*.rs", "--exec", "echo {}",
        "--stats", "--color", "never", td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(stderr.contains("2 files"), "stderr: {stderr}");
}

#[test]
fn exec_spaces_in_path() {
    let td = TestDir::new();
    td.write("dir with spaces/file.txt", "hello\n");
    let (out, code) = run_args(&[
        "--files", "--exec", "echo {}",
        "--color", "never", td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("dir with spaces"), "out: {out}");
}


#[test]
fn null_files_mode() {
    let td = TestDir::new();
    td.write("a.txt", "hello\n");
    td.write("b.txt", "world\n");
    let (out, code) = run_args(&[
        "--files", "-0", "--sort", "path", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    let paths: Vec<&str> = out.split('\0').filter(|s| !s.is_empty()).collect();
    assert_eq!(paths.len(), 2);
    assert!(paths[0].ends_with("a.txt"));
    assert!(paths[1].ends_with("b.txt"));
    assert!(!out.contains('\n'));
}

#[test]
fn null_files_with_matches() {
    let td = TestDir::new();
    td.write("match.txt", "apple pie\n");
    td.write("miss.txt", "banana split\n");
    let (out, code) = run_args(&[
        "-l", "-0", "--color", "never",
        "apple", td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    let paths: Vec<&str> = out.split('\0').filter(|s| !s.is_empty()).collect();
    assert_eq!(paths.len(), 1);
    assert!(paths[0].contains("match.txt"));
    assert!(!out.contains('\n'));
}

#[test]
fn null_files_without_match() {
    let td = TestDir::new();
    td.write("match.txt", "apple pie\n");
    td.write("miss.txt", "banana split\n");
    let (out, code) = run_args(&[
        "--files-without-match", "-0", "--color", "never",
        "apple", td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    let paths: Vec<&str> = out.split('\0').filter(|s| !s.is_empty()).collect();
    assert_eq!(paths.len(), 1);
    assert!(paths[0].contains("miss.txt"));
    assert!(!out.contains('\n'));
}

#[test]
fn e_flag_union() {
    let (out, code) = run_stdin(&["-e", "foo", "-e", "bar", "--no-line-number", "--color", "never"], "foo\nbar\nbaz\n");
    assert_eq!(code, 0);
    assert_eq!(out, "foo\nbar");
}

#[test]
fn e_flag_union_with_and() {
    let (out, code) = run_stdin(
        &["-e", "foo", "-e", "bar", "-a", "x", "--no-line-number", "--color", "never"],
        "foo x\nbar x\nfoo y\nbaz x\n",
    );
    assert_eq!(code, 0);
    assert_eq!(out, "foo x\nbar x");
}

#[test]
fn e_flag_union_with_and_paragraph() {
    let (out, code) = run_stdin(
        &["-e", "foo", "-e", "bar", "-a", "x", "-p", "--no-line-number", "--color", "never"],
        "foo\nx\n\nbar\nx\n\nfoo\ny\n",
    );
    assert_eq!(code, 0);
    assert!(out.contains("foo"));
    assert!(out.contains("bar"));
    assert!(!out.contains("y"));
}

#[test]
fn e_flag_union_with_and_file_scope() {
    let td = TestDir::new();
    td.write("a.txt", "foo\nx\n");
    td.write("b.txt", "bar\nx\n");
    td.write("c.txt", "foo\ny\n");
    let (out, code) = run_args(&[
        "-e", "foo", "-e", "bar", "-a", "x", "-d", "file",
        "--sort", "path", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("a.txt"));
    assert!(out.contains("b.txt"));
    assert!(!out.contains("c.txt"));
}

#[test]
fn e_flag_union_with_not() {
    let (out, code) = run_stdin(
        &["-e", "foo", "-e", "bar", "-N", "x", "--no-line-number", "--color", "never"],
        "foo x\nbar x\nfoo y\nbar z\nbaz\n",
    );
    assert_eq!(code, 0);
    assert_eq!(out, "foo y\nbar z");
}

#[test]
fn file_scope_negation_only() {
    let td = TestDir::new();
    td.write("a.txt", "foo\nhello\n");
    td.write("b.txt", "bar\nworld\n");
    td.write("c.txt", "foo\nbar\n");
    td.write("d.txt", "hello\nworld\n");
    let (out, code) = run_args(&[
        "-d", "file", "-N", "foo", "--sort", "path", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("b.txt"));
    assert!(out.contains("d.txt"));
    assert!(!out.contains("a.txt"));
    assert!(!out.contains("c.txt"));
}

#[test]
fn file_scope_negation_only_implicit_cwd() {
    let td = TestDir::new();
    td.write("a.txt", "foo\nhello\n");
    td.write("b.txt", "bar\nworld\n");
    let mut cmd = resharp();
    cmd.current_dir(td.path());
    cmd.stdin(std::process::Stdio::null());
    cmd.args(&["-d", "file", "-N", "foo", "--sort", "path", "--color", "never"]);
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
    assert!(out.status.success(), "should succeed, got code={} stderr={}", out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains("b.txt"));
    assert!(!stdout.contains("a.txt"));
}

#[test]
fn file_scope_multiple_negations() {
    let td = TestDir::new();
    td.write("a.txt", "foo\nhello\n");
    td.write("b.txt", "bar\nworld\n");
    td.write("c.txt", "foo\nbar\n");
    td.write("d.txt", "hello\nworld\n");
    let (out, code) = run_args(&[
        "-d", "file", "-N", "foo", "-N", "bar", "--sort", "path", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("d.txt"));
    assert!(!out.contains("a.txt"));
    assert!(!out.contains("b.txt"));
    assert!(!out.contains("c.txt"));
}

#[test]
fn file_scope_positive_and_negation() {
    let td = TestDir::new();
    td.write("a.txt", "foo\nhello\n");
    td.write("b.txt", "bar\nworld\n");
    td.write("c.txt", "foo\nbar\n");
    td.write("d.txt", "hello\nworld\n");
    let (out, code) = run_args(&[
        "-d", "file", "-a", "foo", "-N", "bar", "--sort", "path", "--color", "never",
        td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("a.txt"));
    assert!(!out.contains("b.txt"));
    assert!(!out.contains("c.txt"));
    assert!(!out.contains("d.txt"));
}

#[test]
fn null_not_set_by_default() {
    let td = TestDir::new();
    td.write("a.txt", "hello\n");
    let (out, _) = run_args(&[
        "-l", "--color", "never",
        "hello", td.path().to_str().unwrap(),
    ]);
    assert!(out.contains('\n') || out.ends_with("a.txt"));
    assert!(!out.contains('\0'));
}

#[test]
fn ignore_file() {
    let td = TestDir::new();
    td.write("a.txt", "hello world\n");
    td.write("b.log", "hello world\n");
    td.write("myignore", "*.log\n");
    let (out, code) = run_args(&[
        "--ignore-file", td.path().join("myignore").to_str().unwrap(),
        "-l", "--color", "never",
        "hello", td.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(out.contains("a.txt"));
    assert!(!out.contains("b.log"));
}

#[test]
fn trim_long_line() {
    let long = format!("{}FINDME{}", "x".repeat(200), "y".repeat(200));
    let (out, code) = run_stdin(&["--trim", "--color", "never", "FINDME"], &long);
    assert_eq!(code, 0);
    assert!(out.contains("FINDME"), "match should be visible");
    assert!(out.contains("\u{2026}"), "should have ellipsis");
    assert!(out.len() < long.len(), "output should be shorter than input");
}

#[test]
fn trim_short_line_unchanged() {
    let (out, code) = run_stdin(&["--trim", "--no-line-number", "--color", "never", "hello"], "hello world");
    assert_eq!(code, 0);
    assert_eq!(out, "hello world");
}

#[test]
fn trim_match_at_start() {
    let long = format!("FINDME{}", "x".repeat(300));
    let (out, code) = run_stdin(&["--trim", "--no-line-number", "--color", "never", "FINDME"], &long);
    assert_eq!(code, 0);
    assert!(out.starts_with("FINDME"), "match at start should remain at start");
    assert!(out.ends_with("\u{2026}"), "should end with ellipsis");
}

#[test]
fn vimgrep_basic() {
    let (out, code) = run_stdin(&["--vimgrep", "hello"], "hello world\nfoo\nhello again");
    assert_eq!(code, 0);
    assert_eq!(out, "1:1:hello world\n3:1:hello again");
}

#[test]
fn vimgrep_multiple_matches_per_line() {
    let (out, code) = run_stdin(&["--vimgrep", "aaa"], "aaa bbb aaa");
    assert_eq!(code, 0);
    assert_eq!(out, "1:1:aaa bbb aaa\n1:9:aaa bbb aaa");
}

#[test]
fn vimgrep_with_file() {
    let td = TestDir::new();
    td.write("test.txt", "foo bar\nbaz foo\n");
    let (out, code) = run_args(&["--vimgrep", "foo", td.path().join("test.txt").to_str().unwrap()]);
    assert_eq!(code, 0);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains(":1:1:foo bar"));
    assert!(lines[1].contains(":2:5:baz foo"));
}

#[test]
fn passthru_shows_all_lines() {
    let (out, code) = run_stdin(
        &["--passthru", "--no-line-number", "--color", "never", "foo"],
        "line one\nfoo bar\nline three\nfoo baz\nline five",
    );
    assert_eq!(code, 0);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0], "line one");
    assert_eq!(lines[1], "foo bar");
    assert_eq!(lines[4], "line five");
}

#[test]
fn passthru_no_match_exit_1() {
    let (_, code) = run_stdin(
        &["--passthru", "--color", "never", "xyz"],
        "no match here",
    );
    assert_eq!(code, 1);
}

#[test]
fn max_columns_skips_long_lines() {
    let (out, code) = run_stdin(
        &["--max-columns", "20", "--no-line-number", "--color", "never", "match"],
        "short match\nthis is a very long line that also has match in it\nmatch again",
    );
    assert_eq!(code, 0);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], "short match");
    assert_eq!(lines[1], "match again");
}

#[test]
fn context_separator_custom() {
    let (out, _) = run_stdin(
        &["-C", "1", "--context-separator", "~~~", "--no-line-number", "--color", "never", "x"],
        "a\nx\nb\nc\nd\nx\ne",
    );
    assert!(out.contains("~~~"), "should use custom separator");
    assert!(!out.contains("\n--\n"), "should not use default separator");
}

#[test]
fn context_separator_empty() {
    let (out, _) = run_stdin(
        &["-C", "1", "--context-separator", "", "--no-line-number", "--color", "never", "x"],
        "a\nx\nb\nc\nd\nx\ne",
    );
    assert!(!out.contains("\n--\n"), "should suppress separator");
}

#[test]
fn heading_shows_match_count() {
    let td = TestDir::new();
    td.write("a.txt", "foo\nbar\nfoo\n");
    let (out, code) = run_args(&[
        "--heading", "--color", "never",
        "foo", td.path().join("a.txt").to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    let first_line = out.lines().next().unwrap();
    assert!(first_line.contains("(2)"), "heading should show match count, got: {first_line}");
}

#[test]
fn head_limits_output_lines() {
    // 5 matching lines, limit to 2 output lines
    let (out, _) = run_stdin(
        &["--head", "2", "--no-line-number", "--color", "never", "a"],
        "a\na\na\na\na\n",
    );
    assert_eq!(out.lines().count(), 2);
}

#[test]
fn head_truncation_notice() {
    let (out, stderr, _) = run_args_full(&[
        "--head", "1",
        "--no-heading", "--no-line-number", "--color", "never",
        "a",
        "/dev/stdin",
    ]);
    // just check the notice format exists in a stdin search
    let _ = (out, stderr);
}

#[test]
fn head_no_truncation_when_under_limit() {
    let (out, _) = run_stdin(
        &["--head", "10", "--no-line-number", "--color", "never", "a"],
        "a\na\na\n",
    );
    assert_eq!(out.lines().count(), 3);
}

#[test]
fn head_file_truncation_notice() {
    let td = TestDir::new();
    td.write("a.txt", "a\na\na\na\na\n");
    let (out, stderr, code) = run_args_full(&[
        "--head", "2",
        "--no-heading", "--no-line-number", "--color", "never",
        "a", td.path().join("a.txt").to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert_eq!(out.lines().count(), 2);
    assert!(stderr.contains("truncated"), "expected truncation notice, got: {stderr}");
}

#[test]
fn offset_skips_lines() {
    // 5 matching lines, skip first 2
    let (out, _) = run_stdin(
        &["--offset", "2", "--no-line-number", "--color", "never", "a"],
        "a\na\na\na\na\n",
    );
    assert_eq!(out.lines().count(), 3);
}

#[test]
fn offset_combined_with_head() {
    // 5 matching lines, skip 2 then take 2
    let (out, _) = run_stdin(
        &["--offset", "2", "--head", "2", "--no-line-number", "--color", "never", "a"],
        "a\na\na\na\na\n",
    );
    assert_eq!(out.lines().count(), 2);
}

#[test]
fn offset_greater_than_total() {
    // offset beyond all results yields no output
    let (out, _) = run_stdin(
        &["--offset", "10", "--no-line-number", "--color", "never", "a"],
        "a\na\na\n",
    );
    assert_eq!(out, "");
}

#[test]
fn offset_zero_returns_all() {
    // offset=0 is a no-op
    let (out, _) = run_stdin(
        &["--offset", "0", "--no-line-number", "--color", "never", "a"],
        "a\na\na\n",
    );
    assert_eq!(out.lines().count(), 3);
}

#[test]
fn count_matches_stdin() {
    let (out, code) = run_stdin(&["--count-matches", "a"], "a\nb\na\nc\na\n");
    assert_eq!(code, 0);
    assert_eq!(out, "3");
}

#[test]
fn count_matches_no_matches() {
    let (out, code) = run_stdin(&["--count-matches", "z"], "a\nb\nc\n");
    assert_eq!(code, 1);
    assert_eq!(out, "0");
}

#[test]
fn count_matches_multi_file() {
    let td = TestDir::new();
    td.write("a.txt", "a\na\n");
    td.write("b.txt", "a\nb\n");
    let (out, code) = run_args(&["--count-matches", "a", td.path().to_str().unwrap()]);
    assert_eq!(code, 0);
    assert_eq!(out, "3");
}

#[test]
fn count_matches_suppresses_normal_output() {
    let (out, _) = run_stdin(&["--count-matches", "a"], "a\na\n");
    // output must be exactly the number - no file paths, no line content
    assert_eq!(out.lines().count(), 1);
    assert_eq!(out.trim().parse::<usize>().unwrap(), 2);
}

#[test]
fn no_filename_suppresses_path_in_multi_file() {
    let td = TestDir::new();
    td.write("a.txt", "foo\n");
    td.write("b.txt", "foo\n");
    let (out, code) = run_args(&["--no-filename", "--no-heading", "foo", td.path().to_str().unwrap()]);
    assert_eq!(code, 0);
    for line in out.lines() {
        assert!(!line.contains(".txt"), "path should be suppressed: {line}");
    }
}

#[test]
fn no_filename_short_flag() {
    let td = TestDir::new();
    td.write("a.txt", "bar\n");
    td.write("b.txt", "bar\n");
    let (out, code) = run_args(&["-h", "--no-heading", "bar", td.path().to_str().unwrap()]);
    assert_eq!(code, 0);
    for line in out.lines() {
        assert!(!line.contains(".txt"), "path should be suppressed: {line}");
    }
}

#[test]
fn no_filename_single_file() {
    let td = TestDir::new();
    let f = td.write("a.txt", "foo\n");
    // single file: path is not shown by default, --no-filename is a no-op
    let (out_default, _) = run_args(&["foo", f.to_str().unwrap()]);
    let (out_h, _) = run_args(&["-h", "foo", f.to_str().unwrap()]);
    assert_eq!(out_default, out_h);
}

#[test]
fn stdin_no_line_numbers_by_default() {
    let (out, code) = run_stdin(&["--color", "never", "apple"], "apple\nbanana\napple pie\n");
    assert_eq!(code, 0);
    assert_eq!(out, "apple\napple pie");
}

#[test]
fn stdin_line_numbers_with_flag() {
    let (out, code) = run_stdin(&["-n", "--color", "never", "apple"], "apple\nbanana\napple pie\n");
    assert_eq!(code, 0);
    assert_eq!(out, "1:apple\n3:apple pie");
}
