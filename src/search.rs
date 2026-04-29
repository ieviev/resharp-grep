use std::io::{Read, Write};
use std::path::Path;

use crate::args::Args;
use crate::printer::{self, PrinterOpts};

pub struct LineMatch {
    pub line_number: usize,
    pub line_start: usize,
    pub match_ranges: Vec<(usize, usize)>,
}

pub struct SearchResult {
    pub matches: Vec<LineMatch>,
    pub match_count: usize,
    pub had_error: bool,
}

pub fn build_line_index(buf: &[u8]) -> Vec<usize> {
    let mut starts = vec![0];
    for (i, &b) in buf.iter().enumerate() {
        if b == b'\n' && i + 1 < buf.len() {
            starts.push(i + 1);
        }
    }
    starts
}

fn offset_to_line(line_starts: &[usize], offset: usize) -> usize {
    match line_starts.binary_search(&offset) {
        Ok(i) => i,
        Err(i) => i.saturating_sub(1),
    }
}

fn line_end(buf: &[u8], line_start: usize) -> usize {
    let rest = &buf[line_start..];
    match memchr::memchr(b'\n', rest) {
        Some(pos) => line_start + pos,
        None => buf.len(),
    }
}

pub fn any_not_matches(not_res: &[resharp::Regex], buf: &[u8]) -> bool {
    not_res.iter().any(|re| re.is_match(buf).unwrap_or(false))
}

pub fn search_buffer(
    re: &resharp::Regex,
    highlight_re: Option<&resharp::Regex>,
    buf: &[u8],
    args: &Args,
    effective_max: Option<usize>,
) -> SearchResult {
    if buf.is_empty() {
        return SearchResult {
            matches: vec![],
            match_count: 0,
            had_error: false,
        };
    }

    let all_matches = match re.find_all(buf) {
        Ok(m) => m,
        Err(_) => {
            return SearchResult {
                matches: vec![],
                match_count: 0,
                had_error: true,
            };
        }
    };

    let line_starts = build_line_index(buf);
    let mut line_matches: Vec<LineMatch> = Vec::new();

    for m in &all_matches {
        if m.start == m.end && m.start == buf.len() {
            continue; // skip end-of-input empty match
        }
        // skip zero-width match if previous match ended at this position (find_all stutter on _*/.*)
        if m.start == m.end {
            if let Some(last) = line_matches.last() {
                if let Some(&(_, last_me)) = last.match_ranges.last() {
                    if last.line_number == offset_to_line(&line_starts, m.start)
                        && last.line_start + last_me == m.start
                    {
                        continue;
                    }
                }
            }
        }
        let start_line = offset_to_line(&line_starts, m.start);
        let end_line = if m.end > m.start {
            offset_to_line(&line_starts, m.end.saturating_sub(1))
        } else {
            start_line
        };

        for line_idx in start_line..=end_line {
            let ls = line_starts[line_idx];
            let le = line_end(buf, ls);
            let line_len = le - ls;

            let rel_start = if line_idx == start_line { m.start - ls } else { 0 };
            let rel_end = if line_idx == end_line {
                (m.end - ls).min(line_len)
            } else {
                line_len
            };
            let match_range = (rel_start, rel_end);

            if let Some(last) = line_matches.last_mut() {
                if last.line_number == line_idx {
                    last.match_ranges.push(match_range);
                    continue;
                }
            }

            line_matches.push(LineMatch {
                line_number: line_idx,
                line_start: ls,
                match_ranges: vec![match_range],
            });
        }
    }

    if let Some(hl) = highlight_re {
        for lm in &mut line_matches {
            let ls = lm.line_start;
            let le = line_end(buf, ls);
            let line_buf = &buf[ls..le];
            if let Ok(hl_matches) = hl.find_all(line_buf) {
                if !hl_matches.is_empty() {
                    lm.match_ranges = hl_matches.iter().map(|m| (m.start, m.end)).collect();
                }
            }
        }
    }

    if args.invert_match {
        let matched_lines: std::collections::HashSet<usize> =
            line_matches.iter().map(|m| m.line_number).collect();
        let mut inverted = Vec::new();
        for (i, &ls) in line_starts.iter().enumerate() {
            if !matched_lines.contains(&i) {
                inverted.push(LineMatch {
                    line_number: i,
                    line_start: ls,
                    match_ranges: vec![],
                });
            }
        }
        let count = inverted.len();
        return SearchResult {
            matches: inverted,
            match_count: count,
            had_error: false,
        };
    }

    let max = effective_max.or(args.max_count);
    if let Some(max) = max {
        line_matches.truncate(max);
    }

    let match_count = line_matches.len();

    if args.passthru {
        let mut all_lines = Vec::with_capacity(line_starts.len());
        let mut mi = 0;
        for (i, &ls) in line_starts.iter().enumerate() {
            if mi < line_matches.len() && line_matches[mi].line_number == i {
                all_lines.push(LineMatch {
                    line_number: i,
                    line_start: ls,
                    match_ranges: std::mem::take(&mut line_matches[mi].match_ranges),
                });
                mi += 1;
            } else {
                all_lines.push(LineMatch {
                    line_number: i,
                    line_start: ls,
                    match_ranges: vec![],
                });
            }
        }
        return SearchResult {
            matches: all_lines,
            match_count,
            had_error: false,
        };
    }

    SearchResult {
        matches: line_matches,
        match_count,
        had_error: false,
    }
}

fn read_file(path: &Path, args: &Args) -> anyhow::Result<FileData> {
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    let len = metadata.len();

    if len == 0 {
        return Ok(FileData::Vec(vec![]));
    }

    if args.use_mmap(len) {
        // SAFETY: file is opened read-only, we don't modify the mapping
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Ok(FileData::Mmap(mmap))
    } else {
        Ok(FileData::Vec(std::fs::read(path)?))
    }
}

enum FileData {
    Vec(Vec<u8>),
    Mmap(memmap2::Mmap),
}

impl AsRef<[u8]> for FileData {
    fn as_ref(&self) -> &[u8] {
        match self {
            FileData::Vec(v) => v,
            FileData::Mmap(m) => m,
        }
    }
}

pub fn is_binary(buf: &[u8]) -> bool {
    let check_len = buf.len().min(8192);
    memchr::memchr(0, &buf[..check_len]).is_some()
}

pub fn count_lines(buf: &[u8]) -> usize {
    if buf.is_empty() {
        return 0;
    }
    let newlines = memchr::memchr_iter(b'\n', buf).count();
    if buf.last() == Some(&b'\n') { newlines } else { newlines + 1 }
}

pub fn search_file_to_writer(
    re: &resharp::Regex,
    highlight_re: Option<&resharp::Regex>,
    not_res: &[resharp::Regex],
    path: &Path,
    args: &Args,
    printer_opts: &PrinterOpts,
    out: &mut dyn termcolor::WriteColor,
    effective_max: Option<usize>,
    unique_set: Option<&mut printer::UniqueSet>,
) -> anyhow::Result<(bool, bool, usize, usize)> {
    let data = read_file(path, args)?;
    let buf = data.as_ref();
    let line_count = count_lines(buf);

    if !args.search_binary() && is_binary(buf) {
        return Ok((false, false, 0, line_count));
    }

    let result = search_buffer(re, highlight_re, buf, args, effective_max);

    if result.had_error {
        eprintln!("resharp: {}: DFA capacity exceeded, skipping", path.display());
        return Ok((false, true, 0, line_count));
    }

    if result.match_count > 0 && any_not_matches(not_res, buf) {
        return Ok((false, false, 0, line_count));
    }

    let match_count = result.match_count;
    let found = match_count > 0;

    if args.quiet || args.count_matches {
        return Ok((found, false, match_count, line_count));
    }

    let path_str = Some(path.to_string_lossy().into_owned());
    printer::write_results_with_unique(out, buf, &result.matches, path_str.as_deref(), printer_opts, unique_set)?;

    Ok((found, false, match_count, line_count))
}

/// Returns the byte offset just after the nth newline in `data`, or `data.len()` if fewer than n newlines.
pub fn find_nth_newline(data: &[u8], n: usize) -> usize {
    let mut count = 0;
    for (i, &b) in data.iter().enumerate() {
        if b == b'\n' {
            count += 1;
            if count >= n {
                return i + 1;
            }
        }
    }
    data.len()
}

/// Applies offset/head pagination to one output chunk.
///
/// Updates `offset_remaining` and `head_remaining` in-place.
/// Returns `(slice_to_write, truncated)` - `None` slice means skip entirely.
pub fn paginate_chunk<'a>(
    output: &'a [u8],
    offset_remaining: &mut usize,
    head_remaining: &mut Option<usize>,
) -> (Option<&'a [u8]>, bool) {
    let file_lines = output.iter().filter(|&&b| b == b'\n').count();

    let (output, file_lines) = if *offset_remaining > 0 {
        if file_lines <= *offset_remaining {
            *offset_remaining -= file_lines;
            return (None, false);
        }
        let skip_to = find_nth_newline(output, *offset_remaining);
        let skipped = output[..skip_to].iter().filter(|&&b| b == b'\n').count();
        *offset_remaining = 0;
        (&output[skip_to..], file_lines - skipped)
    } else {
        (output, file_lines)
    };

    if let Some(ref mut remaining) = head_remaining {
        if *remaining == 0 {
            return (None, file_lines > 0);
        }
        if file_lines <= *remaining {
            *remaining -= file_lines;
            (Some(output), false)
        } else {
            let cut = find_nth_newline(output, *remaining);
            *remaining = 0;
            (Some(&output[..cut]), true)
        }
    } else {
        (Some(output), false)
    }
}

pub fn search_stdin(
    re: &resharp::Regex,
    highlight_re: Option<&resharp::Regex>,
    not_res: &[resharp::Regex],
    args: &Args,
    printer_opts: &PrinterOpts,
    color_choice: termcolor::ColorChoice,
) -> anyhow::Result<(bool, usize)> {
    let mut stdin_buf = Vec::new();
    std::io::stdin().read_to_end(&mut stdin_buf)?;

    let result = search_buffer(re, highlight_re, &stdin_buf, args, args.effective_max(0));

    if result.had_error {
        anyhow::bail!("DFA capacity exceeded");
    }

    if result.match_count > 0 && any_not_matches(not_res, &stdin_buf) {
        return Ok((false, 0));
    }

    let match_count = result.match_count;
    let found = match_count > 0;

    if !args.quiet && !args.count_matches {
        if args.head.is_some() || args.offset.is_some() {
            let bufwtr = termcolor::BufferWriter::stdout(color_choice);
            let mut out_buf = bufwtr.buffer();
            printer::write_results_with_unique(&mut out_buf, &stdin_buf, &result.matches, None, printer_opts, None)?;
            let output = out_buf.as_slice();

            let mut offset_remaining = args.offset.unwrap_or(0);
            let mut head_remaining = args.head;
            let (maybe_out, truncated) = paginate_chunk(output, &mut offset_remaining, &mut head_remaining);
            if let Some(out) = maybe_out {
                std::io::stdout().write_all(out)?;
                std::io::stdout().flush()?;
            }
            if truncated {
                eprintln!("... [truncated at {} lines]", args.head.unwrap());
            }
        } else {
            printer::print_results(&stdin_buf, &result.matches, None, printer_opts, color_choice)?;
        }
    }

    Ok((found, match_count))
}

