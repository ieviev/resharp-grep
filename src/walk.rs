use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use ignore::overrides::OverrideBuilder;
use ignore::types::TypesBuilder;
use ignore::WalkBuilder;
use termcolor::{BufferWriter, ColorSpec, WriteColor};

use crate::args::Args;
use crate::printer::{self, PrinterOpts};
use crate::search;

#[derive(Default)]
pub struct Stats {
    pub files_searched: usize,
    pub files_matched: usize,
    pub match_count: usize,
    pub total_lines: usize,
    pub elapsed: std::time::Duration,
}

pub fn print_type_list() {
    let mut builder = TypesBuilder::new();
    builder.add_defaults();
    let types = builder.build().unwrap();
    for def in types.definitions() {
        let globs = def.globs().join(", ");
        println!("{}: {globs}", def.name());
    }
}

pub fn walk_and_search(
    re: &resharp::Regex,
    highlight_re: Option<&resharp::Regex>,
    pattern: &str,
    highlight_pattern: Option<&str>,
    args: &Args,
    paths: &[PathBuf],
    printer_opts: &PrinterOpts,
    color_choice: termcolor::ColorChoice,
) -> anyhow::Result<(bool, bool, Stats)> {
    let max_filesize = args.parse_max_filesize()?;

    if args.sort.as_deref() == Some("path") {
        return walk_sorted(re, highlight_re, args, paths, printer_opts, color_choice, max_filesize);
    }

    // force sequential for --unique (needs shared dedup state)
    if args.unique {
        let walker = build_walker(args, paths, 1)?;
        return walk_sequential(walker, re, highlight_re, args, printer_opts, color_choice, max_filesize);
    }

    let threads = args.threads.unwrap_or(0);
    let use_parallel = match args.threads {
        Some(n) => n > 1,
        None => std::thread::available_parallelism().map_or(false, |n| n.get() > 1),
    };
    let walker = build_walker(args, paths, threads)?;

    if use_parallel {
        walk_parallel(walker, pattern, highlight_pattern, args, printer_opts, color_choice, max_filesize)
    } else {
        walk_sequential(walker, re, highlight_re, args, printer_opts, color_choice, max_filesize)
    }
}

fn build_walker(
    args: &Args,
    paths: &[PathBuf],
    threads: usize,
) -> anyhow::Result<WalkBuilder> {
    let mut builder = WalkBuilder::new(&paths[0]);
    for p in &paths[1..] {
        builder.add(p);
    }

    builder.hidden(!args.effective_hidden());
    if args.effective_no_ignore() {
        builder.ignore(false);
        builder.git_ignore(false);
        builder.git_global(false);
        builder.git_exclude(false);
    }
    if args.no_ignore_vcs {
        builder.git_ignore(false);
        builder.git_global(false);
        builder.git_exclude(false);
    }
    builder.follow_links(args.follow);
    if let Some(depth) = args.max_depth {
        builder.max_depth(Some(depth));
    }
    if threads > 0 {
        builder.threads(threads);
    }

    if !args.file_type.is_empty() || !args.type_not.is_empty() {
        let mut types = TypesBuilder::new();
        types.add_defaults();
        for t in &args.file_type {
            types.select(t);
        }
        for t in &args.type_not {
            types.negate(t);
        }
        builder.types(types.build()?);
    }

    if !args.glob.is_empty() || !args.iglob.is_empty() {
        let mut overrides = OverrideBuilder::new(".");
        for g in &args.glob {
            overrides.add(g)?;
        }
        for g in &args.iglob {
            overrides.case_insensitive(true)?.add(g)?;
        }
        builder.overrides(overrides.build()?);
    }

    Ok(builder)
}

fn walk_sequential(
    walker: WalkBuilder,
    re: &resharp::Regex,
    highlight_re: Option<&resharp::Regex>,
    args: &Args,
    printer_opts: &PrinterOpts,
    color_choice: termcolor::ColorChoice,
    max_filesize: Option<u64>,
) -> anyhow::Result<(bool, bool, Stats)> {
    let mut found_any = false;
    let mut had_errors = false;
    let mut stats = Stats::default();
    let mut unique_set = if args.unique { Some(printer::UniqueSet::new()) } else { None };

    for entry in walker.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("resharp: {err}");
                continue;
            }
        };

        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }

        if let Some(max) = max_filesize {
            if entry.metadata().map_or(false, |m| m.len() > max) {
                continue;
            }
        }

        stats.files_searched += 1;
        let effective_max = compute_effective_max(args, stats.match_count);

        let (found, had_error, count, lines) =
            search::search_file(
                re, highlight_re, entry.path(), args, printer_opts, color_choice,
                effective_max, unique_set.as_mut(),
            )?;

        stats.total_lines += lines;

        if had_error {
            had_errors = true;
        }
        if found {
            found_any = true;
            stats.files_matched += 1;
            if args.quiet {
                return Ok((true, had_errors, stats));
            }
        }

        stats.match_count += count;
        if args.max_total.map_or(false, |mt| stats.match_count >= mt) {
            break;
        }
    }

    Ok((found_any, had_errors, stats))
}

fn walk_parallel(
    walker: WalkBuilder,
    pattern: &str,
    highlight_pattern: Option<&str>,
    args: &Args,
    printer_opts: &PrinterOpts,
    color_choice: termcolor::ColorChoice,
    max_filesize: Option<u64>,
) -> anyhow::Result<(bool, bool, Stats)> {
    let bufwtr = Arc::new(BufferWriter::stdout(color_choice));
    let found_any = Arc::new(AtomicBool::new(false));
    let had_errors = Arc::new(AtomicBool::new(false));
    let total_matches = Arc::new(AtomicUsize::new(0));
    let files_searched = Arc::new(AtomicUsize::new(0));
    let files_matched = Arc::new(AtomicUsize::new(0));
    let total_lines = Arc::new(AtomicUsize::new(0));
    let quiet = args.quiet;
    let max_total = args.max_total;
    let pattern = pattern.to_string();
    let highlight_pattern = highlight_pattern.map(|s| s.to_string());
    let dfa_threshold = args.dfa_threshold;
    let dfa_capacity = args.dfa_capacity;

    walker.build_parallel().run(|| {
        let found_any = Arc::clone(&found_any);
        let had_errors = Arc::clone(&had_errors);
        let total_matches = Arc::clone(&total_matches);
        let files_searched = Arc::clone(&files_searched);
        let files_matched = Arc::clone(&files_matched);
        let total_lines = Arc::clone(&total_lines);
        let bufwtr = Arc::clone(&bufwtr);
        let re = match resharp::Regex::with_options(&pattern, resharp::EngineOptions {
            dfa_threshold,
            max_dfa_capacity: dfa_capacity,
            ..Default::default()
        }) {
            Ok(re) => re,
            Err(e) => {
                eprintln!("resharp: failed to compile pattern: {e}");
                return Box::new(move |_| ignore::WalkState::Quit);
            }
        };
        let highlight_re = highlight_pattern.as_ref().and_then(|hp| {
            resharp::Regex::with_options(hp, resharp::EngineOptions {
                dfa_threshold,
                max_dfa_capacity: dfa_capacity,
                ..Default::default()
            }).ok()
        });
        Box::new(move |entry| {
            if quiet && found_any.load(Ordering::Relaxed) {
                return ignore::WalkState::Quit;
            }

            if let Some(mt) = max_total {
                if total_matches.load(Ordering::Relaxed) >= mt {
                    return ignore::WalkState::Quit;
                }
            }

            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    eprintln!("resharp: {err}");
                    return ignore::WalkState::Continue;
                }
            };

            if !entry.file_type().map_or(false, |ft| ft.is_file()) {
                return ignore::WalkState::Continue;
            }

            if let Some(max) = max_filesize {
                if entry.metadata().map_or(false, |m| m.len() > max) {
                    return ignore::WalkState::Continue;
                }
            }

            files_searched.fetch_add(1, Ordering::Relaxed);

            let effective_max = max_total.map(|mt| {
                let current = total_matches.load(Ordering::Relaxed);
                mt.saturating_sub(current)
            });

            let mut buf = bufwtr.buffer();
            match search::search_file_to_writer(&re, highlight_re.as_ref(), entry.path(), args, printer_opts, &mut buf, effective_max) {
                Ok((found, had_error, count, lines)) => {
                    total_lines.fetch_add(lines, Ordering::Relaxed);
                    if had_error {
                        had_errors.store(true, Ordering::Relaxed);
                    }
                    if found {
                        found_any.store(true, Ordering::Relaxed);
                        files_matched.fetch_add(1, Ordering::Relaxed);
                    }
                    total_matches.fetch_add(count, Ordering::Relaxed);
                    if !buf.as_slice().is_empty() {
                        let _ = bufwtr.print(&buf);
                    }
                }
                Err(err) => {
                    eprintln!("resharp: {}: {err}", entry.path().display());
                }
            }

            ignore::WalkState::Continue
        })
    });

    let stats = Stats {
        files_searched: files_searched.load(Ordering::Relaxed),
        files_matched: files_matched.load(Ordering::Relaxed),
        match_count: total_matches.load(Ordering::Relaxed),
        total_lines: total_lines.load(Ordering::Relaxed),
        ..Default::default()
    };

    Ok((found_any.load(Ordering::Relaxed), had_errors.load(Ordering::Relaxed), stats))
}

fn walk_sorted(
    re: &resharp::Regex,
    highlight_re: Option<&resharp::Regex>,
    args: &Args,
    paths: &[PathBuf],
    printer_opts: &PrinterOpts,
    color_choice: termcolor::ColorChoice,
    max_filesize: Option<u64>,
) -> anyhow::Result<(bool, bool, Stats)> {
    let walker = build_walker(args, paths, 1)?;
    let mut entries: Vec<PathBuf> = Vec::new();

    for entry in walker.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("resharp: {err}");
                continue;
            }
        };

        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }

        if let Some(max) = max_filesize {
            if entry.metadata().map_or(false, |m| m.len() > max) {
                continue;
            }
        }

        entries.push(entry.into_path());
    }

    entries.sort();

    let mut found_any = false;
    let mut had_errors = false;
    let mut stats = Stats::default();
    let mut unique_set = if args.unique { Some(printer::UniqueSet::new()) } else { None };

    for path in &entries {
        stats.files_searched += 1;
        let effective_max = compute_effective_max(args, stats.match_count);

        let (found, had_error, count, lines) = search::search_file(
            re, highlight_re, path, args, printer_opts, color_choice,
            effective_max, unique_set.as_mut(),
        )?;
        stats.total_lines += lines;
        if had_error {
            had_errors = true;
        }
        if found {
            found_any = true;
            stats.files_matched += 1;
            if args.quiet {
                return Ok((true, had_errors, stats));
            }
        }

        stats.match_count += count;
        if args.max_total.map_or(false, |mt| stats.match_count >= mt) {
            break;
        }
    }

    Ok((found_any, had_errors, stats))
}

pub fn walk_list_files(
    args: &Args,
    paths: &[PathBuf],
    color_choice: termcolor::ColorChoice,
) -> anyhow::Result<(bool, bool, Stats)> {
    let max_filesize = args.parse_max_filesize()?;
    let walker = build_walker(args, paths, 1)?;
    let bufwtr = BufferWriter::stdout(color_choice);
    let mut stats = Stats::default();

    let sorted = args.sort.as_deref() == Some("path");
    let mut entries: Vec<PathBuf> = Vec::new();
    let count_lines = args.stats;

    for entry in walker.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("resharp: {err}");
                continue;
            }
        };

        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }

        if let Some(max) = max_filesize {
            if entry.metadata().map_or(false, |m| m.len() > max) {
                continue;
            }
        }

        if sorted {
            entries.push(entry.into_path());
        } else {
            stats.files_matched += 1;
            if count_lines {
                if let Ok(buf) = std::fs::read(entry.path()) {
                    stats.total_lines += search::count_lines(&buf);
                }
            }
            print_file_path(&bufwtr, entry.path(), args.null)?;
        }
    }

    if sorted {
        entries.sort();
        for path in &entries {
            stats.files_matched += 1;
            if count_lines {
                if let Ok(buf) = std::fs::read(path) {
                    stats.total_lines += search::count_lines(&buf);
                }
            }
            print_file_path(&bufwtr, path, args.null)?;
        }
    }

    let found_any = stats.files_matched > 0;
    Ok((found_any, false, stats))
}

fn print_file_path(bufwtr: &BufferWriter, path: &std::path::Path, null: bool) -> anyhow::Result<()> {
    let mut buf = bufwtr.buffer();
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    if null {
        write!(buf, "{}\0", abs.display())?;
    } else {
        buf.set_color(ColorSpec::new().set_fg(Some(termcolor::Color::Magenta)))?;
        write!(buf, "{}", abs.display())?;
        buf.reset()?;
        writeln!(buf)?;
    }
    bufwtr.print(&buf)?;
    Ok(())
}

fn compute_effective_max(args: &Args, total_so_far: usize) -> Option<usize> {
    match (args.max_count, args.max_total) {
        (Some(mc), Some(mt)) => Some(mc.min(mt.saturating_sub(total_so_far))),
        (Some(mc), None) => Some(mc),
        (None, Some(mt)) => Some(mt.saturating_sub(total_so_far)),
        (None, None) => None,
    }
}

/// collect file paths matching globs/types (for --files --exec)
pub fn collect_files(
    args: &Args,
    paths: &[PathBuf],
) -> anyhow::Result<(Vec<PathBuf>, Stats)> {
    let max_filesize = args.parse_max_filesize()?;
    let walker = build_walker(args, paths, 1)?;
    let mut stats = Stats::default();
    let mut result = Vec::new();
    let count_lines = args.stats;

    for entry in walker.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("resharp: {err}");
                continue;
            }
        };

        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }

        if let Some(max) = max_filesize {
            if entry.metadata().map_or(false, |m| m.len() > max) {
                continue;
            }
        }

        let path = entry.into_path();
        stats.files_matched += 1;
        if count_lines {
            if let Ok(buf) = std::fs::read(&path) {
                stats.total_lines += search::count_lines(&buf);
            }
        }
        result.push(std::path::absolute(&path).unwrap_or(path));
    }

    if args.sort.as_deref() == Some("path") {
        result.sort();
    }

    Ok((result, stats))
}

/// collect files where pattern matches (for search --exec)
pub fn collect_matching_files(
    re: &resharp::Regex,
    args: &Args,
    paths: &[PathBuf],
) -> anyhow::Result<(Vec<PathBuf>, Stats)> {
    let max_filesize = args.parse_max_filesize()?;
    let walker = build_walker(args, paths, 1)?;
    let mut stats = Stats::default();
    let mut result = Vec::new();

    for entry in walker.build() {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                eprintln!("resharp: {err}");
                continue;
            }
        };

        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }

        if let Some(max) = max_filesize {
            if entry.metadata().map_or(false, |m| m.len() > max) {
                continue;
            }
        }

        stats.files_searched += 1;

        let data = match std::fs::read(entry.path()) {
            Ok(d) => d,
            Err(err) => {
                eprintln!("resharp: {}: {err}", entry.path().display());
                continue;
            }
        };

        stats.total_lines += search::count_lines(&data);

        if !args.search_binary() && data.len().min(8192) > 0
            && memchr::memchr(0, &data[..data.len().min(8192)]).is_some()
        {
            continue;
        }

        let sr = search::search_buffer(re, None, &data, args, None);
        if sr.had_error {
            eprintln!("resharp: {}: DFA capacity exceeded, skipping", entry.path().display());
            continue;
        }

        stats.match_count += sr.matches.len();
        if !sr.matches.is_empty() {
            stats.files_matched += 1;
            let path = entry.into_path();
            result.push(std::path::absolute(&path).unwrap_or(path));
        }
    }

    if args.sort.as_deref() == Some("path") {
        result.sort();
    }

    Ok((result, stats))
}

/// run a command template on each file via sh -c, replacing {} with escaped path
pub fn exec_on_files(
    template: &str,
    files: &[PathBuf],
) -> anyhow::Result<bool> {
    let mut all_ok = true;

    for path in files {
        let escaped = shell_escape(&path.to_string_lossy());
        let cmd = if template.contains("{}") {
            template.replace("{}", &escaped)
        } else {
            format!("{template} {escaped}")
        };

        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .status()
            .map_err(|e| anyhow::anyhow!("exec: {e}"))?;

        if !status.success() {
            all_ok = false;
        }
    }

    Ok(all_ok)
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
