use crate::errors::{AppError, AppResult};
use crate::extractor::RunSummary;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn write_manifest(path: &Path, summary: &RunSummary) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| {
                AppError::manifest(
                    format!("could not create manifest directory {}", parent.display()),
                    err,
                )
            })?;
        }
    }

    fs::write(path, to_json(summary)).map_err(|err| {
        AppError::manifest(format!("could not write manifest {}", path.display()), err)
    })
}

fn to_json(summary: &RunSummary) -> String {
    let mut json = String::new();
    json.push_str("{\n");
    json.push_str("  \"tool\": \"iso-extract-jpegs\",\n");
    json.push_str("  \"version\": ");
    json.push_str(&json_string(env!("CARGO_PKG_VERSION")));
    json.push_str(",\n");
    json.push_str("  \"finished_at_unix\": ");
    json.push_str(&unix_timestamp().to_string());
    json.push_str(",\n");
    json.push_str("  \"dry_run\": ");
    json.push_str(if summary.dry_run { "true" } else { "false" });
    json.push_str(",\n");
    json.push_str("  \"summary\": {\n");
    json.push_str(&format!(
        "    \"inputs\": {},\n    \"files_scanned\": {},\n    \"candidates_found\": {},\n    \"extracted\": {},\n    \"would_extract\": {},\n    \"skipped\": {},\n    \"failed\": {}\n",
        summary.inputs.len(),
        summary.files_scanned,
        summary.candidates_found,
        summary.extracted,
        summary.would_extract,
        summary.skipped,
        summary.failed
    ));
    json.push_str("  },\n");

    json.push_str("  \"inputs\": [\n");
    for (index, input) in summary.inputs.iter().enumerate() {
        if index > 0 {
            json.push_str(",\n");
        }
        json.push_str("    {\n");
        json.push_str("      \"iso\": ");
        json.push_str(&json_string(&input.source_iso.to_string_lossy()));
        json.push_str(",\n");
        json.push_str(&format!(
            "      \"files_scanned\": {},\n      \"candidates_found\": {},\n      \"extracted\": {},\n      \"would_extract\": {},\n      \"skipped\": {},\n      \"failed\": {}\n",
            input.files_scanned,
            input.candidates_found,
            input.extracted,
            input.would_extract,
            input.skipped,
            input.failed
        ));
        json.push_str("    }");
    }
    json.push_str("\n  ],\n");

    json.push_str("  \"results\": [\n");
    for (index, result) in summary.results.iter().enumerate() {
        if index > 0 {
            json.push_str(",\n");
        }
        json.push_str("    {\n");
        json.push_str("      \"source_iso\": ");
        json.push_str(&json_string(&result.source_iso.to_string_lossy()));
        json.push_str(",\n");
        json.push_str("      \"internal_path\": ");
        json.push_str(&json_string(&result.internal_path));
        json.push_str(",\n");
        json.push_str("      \"output_path\": ");
        if let Some(path) = &result.output_path {
            json.push_str(&json_string(&path.to_string_lossy()));
        } else {
            json.push_str("null");
        }
        json.push_str(",\n");
        json.push_str("      \"status\": ");
        json.push_str(&json_string(result.status.as_str()));
        json.push_str(",\n");
        json.push_str("      \"size\": ");
        json.push_str(&result.size.to_string());
        json.push_str(",\n");
        json.push_str("      \"error\": ");
        if let Some(error) = &result.error {
            json.push_str(&json_string(error));
        } else {
            json.push_str("null");
        }
        json.push_str("\n    }");
    }
    json.push_str("\n  ]\n");
    json.push_str("}\n");
    json
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if ch.is_control() => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_json_strings() {
        assert_eq!(json_string("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
    }
}
