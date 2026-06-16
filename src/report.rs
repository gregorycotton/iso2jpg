use crate::extractor::{ExtractionStatus, RunSummary};
use std::io::{self, Write};

pub fn print_human(summary: &RunSummary, verbose: bool) -> io::Result<()> {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for (index, input) in summary.inputs.iter().enumerate() {
        if index > 0 {
            writeln!(out)?;
        }

        writeln!(out, "Input: {}", input.source_iso.display())?;
        if summary.dry_run {
            writeln!(out, "Mode: dry run")?;
        }
        writeln!(out, "Scanned files: {}", input.files_scanned)?;
        writeln!(out, "Matching candidates: {}", input.candidates_found)?;
        if summary.dry_run {
            writeln!(out, "Would extract: {}", input.would_extract)?;
        } else {
            writeln!(out, "Extracted: {}", input.extracted)?;
        }
        writeln!(out, "Skipped: {}", input.skipped)?;
        writeln!(out, "Failed: {}", input.failed)?;
    }

    if summary.inputs.len() > 1 {
        writeln!(out)?;
        writeln!(out, "Total scanned files: {}", summary.files_scanned)?;
        writeln!(
            out,
            "Total matching candidates: {}",
            summary.candidates_found
        )?;
        if summary.dry_run {
            writeln!(out, "Total would extract: {}", summary.would_extract)?;
        } else {
            writeln!(out, "Total extracted: {}", summary.extracted)?;
        }
        writeln!(out, "Total skipped: {}", summary.skipped)?;
        writeln!(out, "Total failed: {}", summary.failed)?;
    }

    if summary.dry_run && !summary.results.is_empty() {
        writeln!(out)?;
        writeln!(out, "Would extract:")?;
        for result in summary
            .results
            .iter()
            .filter(|result| result.status == ExtractionStatus::WouldExtract)
        {
            writeln!(out, "- {}", result.internal_path)?;
        }
    }

    let failures = summary
        .results
        .iter()
        .filter(|result| {
            matches!(
                result.status,
                ExtractionStatus::FailedValidation
                    | ExtractionStatus::FailedRead
                    | ExtractionStatus::FailedWrite
                    | ExtractionStatus::FailedConversion
            )
        })
        .collect::<Vec<_>>();

    if !failures.is_empty() {
        writeln!(out)?;
        writeln!(out, "Failures:")?;
        for failure in failures {
            let error = failure.error.as_deref().unwrap_or("unknown error");
            writeln!(out, "- {}: {}", failure.internal_path, error)?;
        }
    }

    if verbose {
        let notable = summary
            .results
            .iter()
            .filter(|result| result.status != ExtractionStatus::WouldExtract)
            .collect::<Vec<_>>();
        if !notable.is_empty() {
            writeln!(out)?;
            writeln!(out, "Results:")?;
            for result in notable {
                if let Some(path) = &result.output_path {
                    writeln!(
                        out,
                        "- {} -> {} ({})",
                        result.internal_path,
                        path.display(),
                        result.status
                    )?;
                } else {
                    writeln!(out, "- {} ({})", result.internal_path, result.status)?;
                }
            }
        }
    }

    Ok(())
}
