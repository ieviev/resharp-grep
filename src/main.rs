use std::io::{IsTerminal, Write};
use std::process::ExitCode;
use std::time::Instant;

mod args;
mod printer;
mod search;
mod walk;

fn main() -> ExitCode {
    match run() {
        Ok((_, true, _)) => ExitCode::from(2),
        Ok((true, _, _)) => ExitCode::from(0),
        Ok((false, _, _)) => ExitCode::from(1),
        Err(err) => {
            for cause in err.chain() {
                if let Some(ioerr) = cause.downcast_ref::<std::io::Error>() {
                    if ioerr.kind() == std::io::ErrorKind::BrokenPipe {
                        return ExitCode::from(0);
                    }
                }
            }
            let _ = writeln!(std::io::stderr(), "resharp: {err:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> anyhow::Result<(bool, bool, Option<walk::Stats>)> {
    let args = args::parse()?;

    if let Some(shell) = args.completions {
        let mut cmd = <args::Args as clap::CommandFactory>::command();
        let mut out = std::io::stdout();
        clap_complete::generate(shell, &mut cmd, "re", &mut out);
        clap_complete::generate(shell, &mut cmd, "resharp", &mut out);
        return Ok((true, false, None));
    }

    if args.type_list {
        walk::print_type_list();
        return Ok((true, false, None));
    }

    if args.scope.as_deref() == Some("block") {
        anyhow::bail!("--scope block requires --delimiters (not yet implemented)");
    }
    if args.scope.as_deref() == Some("indent") {
        anyhow::bail!("--scope indent is not yet implemented");
    }

    let color_choice = args.color_choice();
    let start = Instant::now();
    let has_exec = args.exec.is_some();

    // --files mode
    if args.files {
        let paths: Vec<_> = if args.paths.is_empty() {
            vec![".".into()]
        } else {
            args.paths.clone()
        };
        for p in &paths {
            if !p.exists() {
                anyhow::bail!("{}: no such file or directory", p.display());
            }
        }

        if has_exec {
            let (files, mut stats) = walk::collect_files(&args, &paths)?;
            let found = !files.is_empty();
            let exec_ok = walk::exec_on_files(args.exec.as_deref().unwrap(), &files)?;
            if args.stats {
                stats.elapsed = start.elapsed();
                print_stats(&stats, true);
            }
            return Ok((found, !exec_ok, Some(stats)));
        }

        let (found, errors, mut stats) = walk::walk_list_files(&args, &paths, color_choice)?;
        if args.stats {
            stats.elapsed = start.elapsed();
            print_stats(&stats, true);
        }
        return Ok((found, errors, Some(stats)));
    }

    // content search mode
    let pattern = args.resolve_pattern()?;
    let re = resharp::Regex::with_options(&pattern, resharp::EngineOptions {
        dfa_threshold: args.dfa_threshold,
        max_dfa_capacity: args.dfa_capacity,
        ..Default::default()
    }).map_err(|e| anyhow::anyhow!("{e}"))?;

    let highlight_pattern = args.resolve_highlight_pattern();
    let highlight_re = highlight_pattern.as_ref().map(|hp| {
        resharp::Regex::with_options(hp, resharp::EngineOptions {
            dfa_threshold: args.dfa_threshold,
            max_dfa_capacity: args.dfa_capacity,
            ..Default::default()
        }).map_err(|e| anyhow::anyhow!("{e}"))
    }).transpose()?;

    if has_exec {
        let paths: Vec<_> = if args.paths.is_empty() {
            vec![".".into()]
        } else {
            args.paths.clone()
        };
        for p in &paths {
            if !p.exists() {
                anyhow::bail!("{}: no such file or directory", p.display());
            }
        }

        let (files, mut stats) = walk::collect_matching_files(&re, &args, &paths)?;
        let found = !files.is_empty();
        let exec_ok = walk::exec_on_files(args.exec.as_deref().unwrap(), &files)?;
        if args.stats {
            stats.elapsed = start.elapsed();
            print_stats(&stats, false);
        }
        return Ok((found, !exec_ok, Some(stats)));
    }

    let printer_opts = printer::PrinterOpts::from_args(&args);

    if args.paths.is_empty() && !std::io::stdin().is_terminal() {
        let found = search::search_stdin(&re, highlight_re.as_ref(), &args, &printer_opts, color_choice)?;
        return Ok((found, false, None));
    }

    let paths: Vec<_> = if args.paths.is_empty() {
        vec![".".into()]
    } else {
        args.paths.clone()
    };

    for p in &paths {
        if !p.exists() {
            anyhow::bail!("{}: no such file or directory", p.display());
        }
    }

    let (found, errors, mut stats) =
        walk::walk_and_search(&re, highlight_re.as_ref(), &pattern, highlight_pattern.as_deref(), &args, &paths, &printer_opts, color_choice)?;
    if args.stats {
        stats.elapsed = start.elapsed();
        print_stats(&stats, false);
    }
    Ok((found, errors, Some(stats)))
}

fn print_stats(stats: &walk::Stats, file_list_mode: bool) {
    let elapsed = stats.elapsed;
    let time = if elapsed.as_secs() >= 1 {
        format!("{:.2}s", elapsed.as_secs_f64())
    } else {
        format!("{:.1}ms", elapsed.as_secs_f64() * 1000.0)
    };

    if file_list_mode {
        eprintln!("\n{} files, {} lines [{}]",
            stats.files_matched, stats.total_lines, time);
    } else {
        eprintln!("\n{} files searched, {} matched, {} matches, {} lines [{}]",
            stats.files_searched, stats.files_matched,
            stats.match_count, stats.total_lines, time);
    }
}
