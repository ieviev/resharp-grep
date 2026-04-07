use std::collections::HashSet;
use std::io::IsTerminal;

use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

use crate::args::Args;
use crate::search::{self, LineMatch};

pub struct PrinterOpts {
    pub heading: bool,
    pub show_path: bool,
    pub line_number: bool,
    pub only_matching: bool,
    pub count: bool,
    pub files_with_matches: bool,
    pub files_without_match: bool,
    pub column: bool,
    pub byte_offset: bool,
    pub before_ctx: usize,
    pub after_ctx: usize,
    pub replace: Option<String>,
    pub json: bool,
    pub unique: bool,
    pub show_scope: bool,
    pub null: bool,
    pub trim_width: Option<usize>,
    pub vimgrep: bool,
    pub max_columns: Option<usize>,
    pub context_separator: String,
}

impl PrinterOpts {
    pub fn from_args(args: &Args) -> Self {
        let multi = args.paths.len() > 1
            || args.paths.first().map_or(false, |p| p.is_dir())
            || (args.paths.is_empty() && std::io::stdin().is_terminal()); // cwd is a dir, but piped stdin is not multi-file

        if args.vimgrep {
            return Self {
                heading: false,
                show_path: true,
                line_number: true,
                only_matching: args.only_matching,
                count: false,
                files_with_matches: false,
                files_without_match: false,
                column: true,
                byte_offset: false,
                before_ctx: 0,
                after_ctx: 0,
                replace: args.replace.clone(),
                json: false,
                unique: args.unique,
                show_scope: false,
                null: false,
                trim_width: None,
                vimgrep: true,
                max_columns: args.max_columns,
                context_separator: String::new(),
            };
        }

        Self {
            heading: args.show_heading(),
            show_path: multi && !args.show_heading() && !args.no_filename,
            line_number: args.show_line_number(multi),
            only_matching: args.only_matching,
            count: args.count,
            files_with_matches: args.files_with_matches,
            files_without_match: args.files_without_match,
            column: args.column,
            byte_offset: args.byte_offset,
            before_ctx: args.before_ctx(),
            after_ctx: args.after_ctx(),
            replace: args.replace.clone(),
            json: args.json,
            unique: args.unique,
            show_scope: args.show_scope,
            null: args.null,
            trim_width: if args.trim {
                Some(detect_terminal_width())
            } else {
                None
            },
            vimgrep: false,
            max_columns: args.max_columns,
            context_separator: args.context_separator.clone(),
        }
    }
}

#[derive(serde::Serialize)]
struct JsonMatchLine {
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    line_number: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    byte_offset: Option<usize>,
    lines: String,
    submatches: Vec<JsonSubmatch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
}

#[derive(serde::Serialize)]
struct JsonSubmatch {
    #[serde(rename = "match")]
    match_text: String,
    start: usize,
    end: usize,
}

pub fn print_results(
    buf: &[u8],
    matches: &[LineMatch],
    path: Option<&str>,
    opts: &PrinterOpts,
    color_choice: ColorChoice,
) -> anyhow::Result<()> {
    let mut out = StandardStream::stdout(color_choice);
    write_results_with_unique(&mut out, buf, matches, path, opts, None)
}

pub struct UniqueSet {
    seen: HashSet<Vec<u8>>,
}

impl UniqueSet {
    pub fn new() -> Self {
        Self { seen: HashSet::new() }
    }

    fn check(&mut self, key: &[u8]) -> bool {
        if self.seen.contains(key) {
            false
        } else {
            self.seen.insert(key.to_vec());
            true
        }
    }
}

pub fn write_results_with_unique(
    out: &mut dyn WriteColor,
    buf: &[u8],
    matches: &[LineMatch],
    path: Option<&str>,
    opts: &PrinterOpts,
    unique_set: Option<&mut UniqueSet>,
) -> anyhow::Result<()> {
    let mut local_set = if opts.unique && unique_set.is_none() {
        Some(UniqueSet::new())
    } else {
        None
    };
    let unique_set = match unique_set {
        Some(us) => Some(us),
        None => local_set.as_mut(),
    };

    if opts.json {
        return write_json_results(out, buf, matches, path, opts, unique_set);
    }

    if opts.files_without_match || opts.files_with_matches {
        return write_file_mode(out, matches, path, opts);
    }

    if opts.count {
        return write_count_mode(out, matches, path);
    }

    write_text_matches(out, buf, matches, path, opts, unique_set)
}

fn write_file_mode(
    out: &mut dyn WriteColor,
    matches: &[LineMatch],
    path: Option<&str>,
    opts: &PrinterOpts,
) -> anyhow::Result<()> {
    let print_file = (opts.files_without_match && matches.is_empty())
        || (opts.files_with_matches && !matches.is_empty());
    if print_file {
        if let Some(p) = path {
            if opts.null {
                write!(out, "{p}\0")?;
            } else {
                out.set_color(ColorSpec::new().set_fg(Some(Color::Magenta)))?;
                write!(out, "{p}")?;
                out.reset()?;
                writeln!(out)?;
            }
        }
    }
    Ok(())
}

fn write_count_mode(
    out: &mut dyn WriteColor,
    matches: &[LineMatch],
    path: Option<&str>,
) -> anyhow::Result<()> {
    if matches.is_empty() {
        return Ok(());
    }
    if let Some(p) = path {
        out.set_color(ColorSpec::new().set_fg(Some(Color::Magenta)))?;
        write!(out, "{p}")?;
        out.reset()?;
        write!(out, ":")?;
    }
    writeln!(out, "{}", matches.len())?;
    Ok(())
}

fn write_text_matches(
    mut out: &mut dyn WriteColor,
    buf: &[u8],
    matches: &[LineMatch],
    path: Option<&str>,
    opts: &PrinterOpts,
    mut unique_set: Option<&mut UniqueSet>,
) -> anyhow::Result<()> {
    if matches.is_empty() {
        return Ok(());
    }

    if opts.heading {
        if let Some(p) = path {
            let match_count = matches.iter().filter(|m| !m.match_ranges.is_empty()).count();
            out.set_color(ColorSpec::new().set_fg(Some(Color::Magenta)))?;
            write!(out, "{p}")?;
            out.reset()?;
            out.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
            write!(out, " ({match_count})")?;
            out.reset()?;
            writeln!(out)?;
        }
    }

    let line_starts = search::build_line_index(buf);
    let total_lines = line_starts.len();
    let has_context = opts.before_ctx > 0 || opts.after_ctx > 0;
    let match_lines: std::collections::HashSet<usize> =
        matches.iter().map(|m| m.line_number).collect();
    let mut last_printed_line: Option<usize> = None;
    let mut last_scope_line: Option<usize> = None;

    for lm in matches {
        if opts.unique {
            let line = get_line(buf, &line_starts, lm.line_number);
            let key = if opts.only_matching && !lm.match_ranges.is_empty() {
                let (ms, me) = lm.match_ranges[0];
                &line[ms.min(line.len())..me.min(line.len())]
            } else {
                line
            };
            if let Some(us) = unique_set.as_mut() {
                if !us.check(key) {
                    continue;
                }
            }
        }

        if let Some(max_col) = opts.max_columns {
            let line = get_line(buf, &line_starts, lm.line_number);
            if line.len() > max_col {
                continue;
            }
        }

        if opts.show_scope {
            if let Some((scope_line, scope_text)) = find_enclosing_scope(buf, &line_starts, lm.line_number) {
                if last_scope_line != Some(scope_line) {
                    last_scope_line = Some(scope_line);
                    if opts.show_path {
                        if let Some(p) = path {
                            out.set_color(ColorSpec::new().set_fg(Some(Color::Magenta)))?;
                            write!(out, "{p}")?;
                            out.reset()?;
                            write!(out, ":")?;
                        }
                    }
                    out.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)))?;
                    write!(out, "{}:", scope_line + 1)?;
                    out.reset()?;
                    out.set_color(ColorSpec::new().set_fg(Some(Color::Cyan)).set_italic(true))?;
                    writeln!(out, "  {scope_text}")?;
                    out.reset()?;
                }
            }
        }

        let ctx_start = lm.line_number.saturating_sub(opts.before_ctx);
        let ctx_end = (lm.line_number + opts.after_ctx).min(total_lines.saturating_sub(1));

        if has_context {
            if let Some(last) = last_printed_line {
                if ctx_start > last + 1 && !opts.context_separator.is_empty() {
                    writeln!(out, "{}", opts.context_separator)?;
                }
            }
            for line_idx in ctx_start..lm.line_number {
                if last_printed_line.map_or(true, |l| line_idx > l) {
                    print_context_line(&mut out, buf, &line_starts, line_idx, path, opts)?;
                    last_printed_line = Some(line_idx);
                }
            }
        }

        if opts.only_matching {
            let line = get_line(buf, &line_starts, lm.line_number);
            for &(ms, me) in &lm.match_ranges {
                print_prefix(&mut out, path, opts, lm, false)?;
                out.set_color(ColorSpec::new().set_fg(Some(Color::Red)).set_bold(true))?;
                if let Some(ref repl) = opts.replace {
                    out.write_all(repl.as_bytes())?;
                } else {
                    out.write_all(&line[ms..me])?;
                }
                out.reset()?;
                writeln!(out)?;
            }
        } else {
            print_match_line(&mut out, buf, &line_starts, lm, path, opts)?;
        }
        last_printed_line = Some(lm.line_number);

        if has_context {
            for line_idx in (lm.line_number + 1)..=ctx_end {
                if !match_lines.contains(&line_idx) {
                    print_context_line(&mut out, buf, &line_starts, line_idx, path, opts)?;
                }
                last_printed_line = Some(line_idx);
            }
        }
    }

    Ok(())
}

fn write_json_results(
    out: &mut dyn WriteColor,
    buf: &[u8],
    matches: &[LineMatch],
    path: Option<&str>,
    opts: &PrinterOpts,
    mut unique_set: Option<&mut UniqueSet>,
) -> anyhow::Result<()> {
    if opts.files_without_match {
        if matches.is_empty() {
            if let Some(p) = path {
                let obj = serde_json::json!({"type": "file", "path": p});
                writeln!(out, "{}", serde_json::to_string(&obj)?)?;
            }
        }
        return Ok(());
    }

    if opts.files_with_matches {
        if !matches.is_empty() {
            if let Some(p) = path {
                let obj = serde_json::json!({"type": "file", "path": p});
                writeln!(out, "{}", serde_json::to_string(&obj)?)?;
            }
        }
        return Ok(());
    }

    if opts.count {
        if !matches.is_empty() {
            let obj = serde_json::json!({
                "type": "count",
                "path": path,
                "count": matches.len(),
            });
            writeln!(out, "{}", serde_json::to_string(&obj)?)?;
        }
        return Ok(());
    }

    let line_starts = search::build_line_index(buf);

    for lm in matches {
        let line = get_line(buf, &line_starts, lm.line_number);
        let line_text = String::from_utf8_lossy(line).into_owned();

        if opts.unique {
            let key = if opts.only_matching && !lm.match_ranges.is_empty() {
                let (ms, me) = lm.match_ranges[0];
                &line[ms.min(line.len())..me.min(line.len())]
            } else {
                line
            };
            if let Some(ref mut us) = unique_set {
                if !us.check(key) {
                    continue;
                }
            }
        }

        let submatches: Vec<JsonSubmatch> = lm.match_ranges.iter().map(|&(ms, me)| {
            let ms = ms.min(line.len());
            let me = me.min(line.len());
            JsonSubmatch {
                match_text: String::from_utf8_lossy(&line[ms..me]).into_owned(),
                start: ms,
                end: me,
            }
        }).collect();

        let column = lm.match_ranges.first().map(|&(ms, _)| ms + 1);
        let byte_offset = if opts.byte_offset { Some(lm.line_start) } else { None };

        let scope = if opts.show_scope {
            find_enclosing_scope(buf, &line_starts, lm.line_number)
                .map(|(_, text)| text)
        } else {
            None
        };

        let obj = JsonMatchLine {
            path: path.map(|s| s.to_string()),
            line_number: lm.line_number + 1,
            column,
            byte_offset,
            lines: line_text,
            submatches,
            scope,
        };

        writeln!(out, "{}", serde_json::to_string(&obj)?)?;
    }

    Ok(())
}

fn get_line<'a>(buf: &'a [u8], line_starts: &[usize], line_idx: usize) -> &'a [u8] {
    let start = line_starts[line_idx];
    let end = if line_idx + 1 < line_starts.len() {
        line_starts[line_idx + 1]
    } else {
        buf.len()
    };
    let line = &buf[start..end];
    if line.ends_with(b"\r\n") {
        &line[..line.len() - 2]
    } else if line.ends_with(b"\n") {
        &line[..line.len() - 1]
    } else {
        line
    }
}

fn get_indent(buf: &[u8], line_starts: &[usize], line_idx: usize) -> usize {
    let line = get_line(buf, line_starts, line_idx);
    line.iter().take_while(|&&b| b == b' ' || b == b'\t').count()
}

fn find_enclosing_scope(buf: &[u8], line_starts: &[usize], match_line: usize) -> Option<(usize, String)> {
    let match_indent = get_indent(buf, line_starts, match_line);

    for line_idx in (0..match_line).rev() {
        let line = get_line(buf, line_starts, line_idx);

        if line.iter().all(|&b| b == b' ' || b == b'\t' || b == b'\r') {
            continue;
        }

        let indent = line.iter().take_while(|&&b| b == b' ' || b == b'\t').count();

        if indent < match_indent {
            let text = std::str::from_utf8(line).unwrap_or("");
            let trimmed = text.trim();
            if looks_like_scope_marker(trimmed) {
                return Some((line_idx, trimmed.to_string()));
            }
        }
    }
    None
}

fn looks_like_scope_marker(line: &str) -> bool {
    let markers = [
        "fn ", "async fn ", "pub fn ", "pub async fn ", "pub(crate) fn ",
        "pub(super) fn ", "pub(crate) async fn ", "unsafe fn ", "pub unsafe fn ",
        "def ", "async def ",
        "class ", "pub struct ", "struct ", "pub enum ", "enum ",
        "impl ", "impl<", "pub trait ", "trait ",
        "func ", "function ", "method ",
        "pub mod ", "mod ",
        "interface ", "pub type ", "type ",
        "export function ", "export async function ", "export default function ",
        "export class ", "export interface ", "export type ",
    ];
    markers.iter().any(|m| line.starts_with(m))
}

fn print_prefix(
    out: &mut dyn WriteColor,
    path: Option<&str>,
    opts: &PrinterOpts,
    lm: &LineMatch,
    is_context: bool,
) -> anyhow::Result<()> {
    let sep = if is_context { "-" } else { ":" };

    if opts.show_path {
        if let Some(p) = path {
            out.set_color(ColorSpec::new().set_fg(Some(Color::Magenta)))?;
            write!(out, "{p}")?;
            out.reset()?;
            write!(out, "{sep}")?;
        }
    }

    if opts.line_number {
        out.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
        write!(out, "{}", lm.line_number + 1)?;
        out.reset()?;
        write!(out, "{sep}")?;
    }

    if opts.column && !lm.match_ranges.is_empty() {
        out.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
        write!(out, "{}", lm.match_ranges[0].0 + 1)?;
        out.reset()?;
        write!(out, "{sep}")?;
    }

    if opts.byte_offset {
        out.set_color(ColorSpec::new().set_fg(Some(Color::Green)))?;
        write!(out, "{}", lm.line_start)?;
        out.reset()?;
        write!(out, "{sep}")?;
    }

    Ok(())
}

fn detect_terminal_width() -> usize {
    if let Ok(cols) = std::env::var("COLUMNS") {
        if let Ok(n) = cols.parse::<usize>() {
            if n > 0 { return n; }
        }
    }
    #[cfg(unix)]
    {
        #[repr(C)]
        struct Winsize { ws_row: u16, ws_col: u16, ws_xpixel: u16, ws_ypixel: u16 }
        extern "C" {
            fn ioctl(fd: std::ffi::c_int, request: std::ffi::c_ulong, ...) -> std::ffi::c_int;
        }
        #[cfg(target_os = "linux")]
        const TIOCGWINSZ: std::ffi::c_ulong = 0x5413;
        #[cfg(target_os = "macos")]
        const TIOCGWINSZ: std::ffi::c_ulong = 0x40087468;

        let mut ws = Winsize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 };
        let ret = unsafe { ioctl(2, TIOCGWINSZ, &mut ws as *mut Winsize) };
        if ret == 0 && ws.ws_col > 0 {
            return ws.ws_col as usize;
        }
    }
    120
}

fn prefix_width(path: Option<&str>, opts: &PrinterOpts, lm: &LineMatch) -> usize {
    let mut w = 0;
    if opts.show_path {
        if let Some(p) = path {
            w += p.len() + 1;
        }
    }
    if opts.line_number {
        w += count_digits(lm.line_number + 1) + 1;
    }
    if opts.column && !lm.match_ranges.is_empty() {
        w += count_digits(lm.match_ranges[0].0 + 1) + 1;
    }
    if opts.byte_offset {
        w += count_digits(lm.line_start) + 1;
    }
    w
}

fn count_digits(mut n: usize) -> usize {
    if n == 0 { return 1; }
    let mut count = 0;
    while n > 0 { count += 1; n /= 10; }
    count
}

fn trim_line<'a>(
    line: &'a [u8],
    match_ranges: &[(usize, usize)],
    max_width: usize,
) -> (&'a [u8], Vec<(usize, usize)>, bool, bool) {
    if max_width < 4 || line.len() <= max_width {
        return (line, match_ranges.to_vec(), false, false);
    }

    let first_match_end = match_ranges.first().map(|&(_, e)| e).unwrap_or(0);

    if first_match_end < max_width.saturating_sub(1) {
        let end = max_width - 1;
        let trimmed = &line[..end.min(line.len())];
        let adjusted = adjust_match_ranges(match_ranges, 0, end);
        (trimmed, adjusted, false, true)
    } else {
        let first_match_start = match_ranges.first().map(|&(s, _)| s).unwrap_or(0);
        let context = max_width / 4;
        let start = first_match_start.saturating_sub(context);
        let has_left = start > 0;
        let usable = max_width.saturating_sub(if has_left { 2 } else { 1 });
        let end = (start + usable).min(line.len());
        let has_right = end < line.len();
        let trimmed = &line[start..end];
        let adjusted = adjust_match_ranges(match_ranges, start, end);
        (trimmed, adjusted, has_left, has_right)
    }
}

fn adjust_match_ranges(ranges: &[(usize, usize)], offset: usize, end: usize) -> Vec<(usize, usize)> {
    ranges.iter()
        .filter_map(|&(ms, me)| {
            if me <= offset || ms >= end { return None; }
            Some((ms.max(offset) - offset, me.min(end) - offset))
        })
        .collect()
}

fn print_match_line(
    out: &mut dyn WriteColor,
    buf: &[u8],
    line_starts: &[usize],
    lm: &LineMatch,
    path: Option<&str>,
    opts: &PrinterOpts,
) -> anyhow::Result<()> {
    if opts.vimgrep && !lm.match_ranges.is_empty() {
        let line = get_line(buf, line_starts, lm.line_number);
        let line_text = String::from_utf8_lossy(line);
        for &(ms, _) in &lm.match_ranges {
            if let Some(p) = path {
                write!(out, "{p}:")?;
            }
            writeln!(out, "{}:{}:{}", lm.line_number + 1, ms + 1, line_text)?;
        }
        return Ok(());
    }

    print_prefix(out, path, opts, lm, false)?;

    let raw_line = get_line(buf, line_starts, lm.line_number);

    let trimmed;
    let (line, match_ranges, left_ell, right_ell) = if let Some(tw) = opts.trim_width {
        let pw = prefix_width(path, opts, lm);
        let (l, r, le, re) = trim_line(raw_line, &lm.match_ranges, tw.saturating_sub(pw));
        trimmed = r;
        (l, trimmed.as_slice(), le, re)
    } else {
        (raw_line, lm.match_ranges.as_slice(), false, false)
    };

    if left_ell { write!(out, "\u{2026}")?; }

    if match_ranges.is_empty() {
        out.write_all(line)?;
    } else {
        let mut pos = 0;
        for &(ms, me) in match_ranges {
            let ms = ms.min(line.len());
            let me = me.min(line.len());
            if ms > pos {
                out.write_all(&line[pos..ms])?;
            }
            out.set_color(ColorSpec::new().set_fg(Some(Color::Red)).set_bold(true))?;
            if let Some(ref repl) = opts.replace {
                out.write_all(repl.as_bytes())?;
            } else {
                out.write_all(&line[ms..me])?;
            }
            out.reset()?;
            pos = me;
        }
        if pos < line.len() {
            out.write_all(&line[pos..])?;
        }
    }

    if right_ell { write!(out, "\u{2026}")?; }
    writeln!(out)?;
    Ok(())
}

fn print_context_line(
    out: &mut dyn WriteColor,
    buf: &[u8],
    line_starts: &[usize],
    line_idx: usize,
    path: Option<&str>,
    opts: &PrinterOpts,
) -> anyhow::Result<()> {
    let dummy = LineMatch {
        line_number: line_idx,
        line_start: line_starts[line_idx],
        match_ranges: vec![],
    };
    print_prefix(out, path, opts, &dummy, true)?;
    let line = get_line(buf, line_starts, line_idx);
    if let Some(tw) = opts.trim_width {
        let pw = prefix_width(path, opts, &dummy);
        let avail = tw.saturating_sub(pw);
        if line.len() > avail && avail > 1 {
            out.write_all(&line[..avail - 1])?;
            write!(out, "\u{2026}")?;
        } else {
            out.write_all(line)?;
        }
    } else {
        out.write_all(line)?;
    }
    writeln!(out)?;
    Ok(())
}
