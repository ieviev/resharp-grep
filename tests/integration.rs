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
    assert_eq!(out, "1:apple pie\n3:apple sauce");
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
    let (out, _) = run_stdin(&["[ac].*e"], "apple pie\nbanana split\napple sauce\ncherry tart\n");
    assert_eq!(out, "1:apple pie\n3:apple sauce\n4:cherry tart");
}

#[test]
fn regex_anchor_start() {
    let (out, _) = run_stdin(&["^apple"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple pie\n3:apple sauce");
}

#[test]
fn regex_anchor_end() {
    let (out, _) = run_stdin(&["e$"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple pie\n3:apple sauce\n5:grape juice");
}

#[test]
fn no_trailing_newline() {
    let (out, _) = run_stdin(&["newline"], "no newline at end");
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
    let (out, _) = run_stdin(&["-i", "hello"], "hello world\nHello World\nHELLO WORLD\nhello\n");
    assert_eq!(out, "1:hello world\n2:Hello World\n3:HELLO WORLD\n4:hello");
}

#[test]
fn case_sensitive() {
    let (out, _) = run_stdin(&["-s", "hello"], "hello world\nHello World\nHELLO WORLD\nhello\n");
    assert_eq!(out, "1:hello world\n4:hello");
}

#[test]
fn smart_case_lower() {
    let (out, _) = run_stdin(&["-S", "hello"], "hello world\nHello World\nHELLO WORLD\nhello\n");
    assert_eq!(out, "1:hello world\n2:Hello World\n3:HELLO WORLD\n4:hello");
}

#[test]
fn smart_case_upper() {
    let (out, _) = run_stdin(&["-S", "Hello"], "hello world\nHello World\nHELLO WORLD\nhello\n");
    assert_eq!(out, "2:Hello World");
}


#[test]
fn invert_match() {
    let (out, _) = run_stdin(&["-v", "apple|sauce"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "2:banana split\n4:cherry tart\n5:grape juice");
}

#[test]
fn word_match() {
    let (out, _) = run_stdin(&["-w", "cat"], "the cat sat\ncatalog\nthe cat and dog\nscatter\n");
    assert_eq!(out, "1:the cat sat\n3:the cat and dog");
}

#[test]
fn line_match() {
    let (out, _) = run_stdin(&["-x", "hello"], "hello world\nHello World\nHELLO WORLD\nhello\n");
    assert_eq!(out, "4:hello");
}

#[test]
fn max_count() {
    let (out, _) = run_stdin(&["-m", "1", "apple"], "apple pie\nbanana split\napple sauce\n");
    assert_eq!(out, "1:apple pie");
}


#[test]
fn count() {
    let (out, _) = run_stdin(&["-c", "apple"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "2");
}

#[test]
fn only_matching() {
    let (out, _) = run_stdin(&["-o", "apple"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple\n3:apple");
}

#[test]
fn column() {
    let (out, _) = run_stdin(&["--column", "bar"], "foo bar baz\n");
    assert_eq!(out, "1:5:foo bar baz");
}

#[test]
fn byte_offset() {
    let (out, _) = run_stdin(&["-b", "bbb"], "aaa\nbbb\nccc\n");
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
    let (out, _) = run_stdin(&["-A", "1", "banana"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "2:banana split\n3-apple sauce");
}

#[test]
fn before_context() {
    let (out, _) = run_stdin(&["-B", "1", "banana"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1-apple pie\n2:banana split");
}

#[test]
fn context_both() {
    let (out, _) = run_stdin(&["-C", "1", "apple"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple pie\n2-banana split\n3:apple sauce\n4-cherry tart");
}

#[test]
fn context_separator() {
    let (out, _) = run_stdin(&["-C", "1", "pie|juice"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple pie\n2-banana split\n--\n4-cherry tart\n5:grape juice");
}


#[test]
fn multiple_patterns_e() {
    let (out, _) = run_stdin(&["-e", "apple", "-e", "banana"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple pie\n2:banana split\n3:apple sauce");
}

#[test]
fn pattern_file() {
    let td = TestDir::new();
    let pf = td.write("pats.txt", "apple\nbanana\n");
    let (out, _) = run_stdin(
        &["-f", pf.to_str().unwrap()],
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
    let (out, _) = run_stdin(&["-F", "foo.bar"], "foo.bar\nfooXbar\n");
    assert_eq!(out, "1:foo.bar");
}


#[test]
fn wildcard_underscore() {
    let (out, _) = run_stdin(&["_*apple_*"], "apple pie\nbanana split\napple sauce\ncherry tart\ngrape juice\n");
    assert_eq!(out, "1:apple pie\n3:apple sauce");
}

#[test]
fn intersection() {
    let (out, _) = run_stdin(&["(_*cat_*)&(_*the_*)"], "the cat sat\ncatalog\nthe cat and dog\nscatter\n");
    assert_eq!(out, "1:the cat sat\n3:the cat and dog");
}

#[test]
fn intersection_both() {
    let (out, _) = run_stdin(&["(_*cat_*)&(_*dog_*)"], "the cat sat\ncatalog\nthe cat and dog\nscatter\n");
    assert_eq!(out, "3:the cat and dog");
}


#[test]
fn lookahead_positive() {
    let (out, _) = run_stdin(&["(?=.*cat)(?=.*mat).*"], "the cat sat on the mat\nthe dog sat\ncat on mat\n");
    assert_eq!(out, "1:the cat sat on the mat\n3:cat on mat");
}

#[test]
fn lookbehind_positive() {
    let (out, _) = run_stdin(&["(?<=foo)bar"], "foobar\nbazbar\nfooqux\n");
    assert_eq!(out, "1:foobar");
}

#[test]
fn lookahead_with_intersection() {
    // lookahead and resharp intersection should compose
    let (out, _) = run_stdin(&["(?=.*hello)(_*world_*)"], "hello world\nfoo world\nhello bar\n");
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
    let (out, _) = run_stdin(&["--paragraphs", "(_*cats_*)&(_*dogs_*)"], input);
    let lines: Vec<&str> = out.lines().take(2).collect();
    assert_eq!(lines, vec!["1:first paragraph about", "2:cats and dogs together"]);
}


#[test]
fn paragraphs_words_match() {
    let input = "first paragraph about\ncats and dogs together\n\nsecond paragraph about\nfish and birds\n";
    let (out, code) = run_stdin(&["-p", "cats", "-p", "dogs"], input);
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
    td.write("dir/main.rs", "fn main() {}\n");
    td.write("dir/main.py", "def main():\n");
    let (out, _) = run_args(&[
        "-t", "rust", "--no-heading", "--no-line-number", "--color", "never",
        "fn main", td.path().join("dir").to_str().unwrap(),
    ]);
    assert!(out.contains("fn main() {}"));
    assert!(!out.contains("def main"));
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
        resharp::EngineOptions {
            dfa_threshold: 0,
            max_dfa_capacity: 65535,
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
    let (out, _) = run_stdin(&["-W", "cat", "-W", "dog"], "the cat and dog\ncat only\ndog only\n");
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
fn fixed_strings_underscore() {
    let (out, _) = run_stdin(&["-F", "a_b"], "a_b\naxb\nabc\n");
    assert_eq!(out, "1:a_b");
}

#[test]
fn fixed_strings_ampersand() {
    let (out, _) = run_stdin(&["-F", "a&b"], "a&b\nabc\n");
    assert_eq!(out, "1:a&b");
}

#[test]
fn fixed_strings_tilde() {
    let (out, _) = run_stdin(&["-F", "a~b"], "a~b\nabc\n");
    assert_eq!(out, "1:a~b");
}

#[test]
fn fixed_strings_all_meta() {
    let (out, _) = run_stdin(&["-F", "_&~"], "_&~\nabc\n");
    assert_eq!(out, "1:_&~");
}

#[test]
fn fixed_strings_dot_and_underscore() {
    let (out, _) = run_stdin(&["-F", "foo._bar"], "foo._bar\nfooXYbar\nfoo.Xbar\n");
    assert_eq!(out, "1:foo._bar");
}

#[test]
fn raw_underscore_literal() {
    // in raw mode, _ should be literal, not resharp wildcard
    let (out, _) = run_stdin(&["-R", "a_b"], "a_b\naxb\nabc\n");
    assert_eq!(out, "1:a_b");
}

#[test]
fn raw_ampersand_literal() {
    let (out, _) = run_stdin(&["-R", "a&b"], "a&b\nabc\n");
    assert_eq!(out, "1:a&b");
}

#[test]
fn raw_tilde_literal() {
    let (out, _) = run_stdin(&["-R", "a~b"], "a~b\nabc\n");
    assert_eq!(out, "1:a~b");
}

#[test]
fn raw_regex_still_works() {
    // raw mode should still support standard regex
    let (out, _) = run_stdin(&["-R", "a.b"], "a_b\naxb\nabc\n");
    assert_eq!(out, "1:a_b\n2:axb");
}

#[test]
fn raw_backslash_preserved() {
    // \d should still work in raw mode
    let (out, _) = run_stdin(&["-R", r"\d+_\d+"], "3_4\nabc\n12_34\n");
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
