use crate::cli::Config;
use crate::errors::{AppError, AppResult};
use crate::iso::{IsoEntry, IsoEntryKind, IsoImage};
use crate::{jpeg, manifest, paths};
use std::fmt::{self, Display};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtractionStatus {
    Extracted,
    Converted,
    WouldExtract,
    SkippedExisting,
    SkippedInvalidPath,
    FailedValidation,
    FailedRead,
    FailedWrite,
    FailedConversion,
}

impl ExtractionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Extracted => "extracted",
            Self::Converted => "converted",
            Self::WouldExtract => "would_extract",
            Self::SkippedExisting => "skipped_existing",
            Self::SkippedInvalidPath => "skipped_invalid_path",
            Self::FailedValidation => "failed_validation",
            Self::FailedRead => "failed_read",
            Self::FailedWrite => "failed_write",
            Self::FailedConversion => "failed_conversion",
        }
    }

    fn is_skipped(&self) -> bool {
        matches!(self, Self::SkippedExisting | Self::SkippedInvalidPath)
    }

    fn is_failed(&self) -> bool {
        matches!(
            self,
            Self::FailedValidation | Self::FailedRead | Self::FailedWrite | Self::FailedConversion
        )
    }
}

impl Display for ExtractionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone)]
pub struct ExtractionResult {
    pub source_iso: PathBuf,
    pub internal_path: String,
    pub output_path: Option<PathBuf>,
    pub status: ExtractionStatus,
    pub size: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InputSummary {
    pub source_iso: PathBuf,
    pub files_scanned: u64,
    pub candidates_found: u64,
    pub extracted: u64,
    pub would_extract: u64,
    pub skipped: u64,
    pub failed: u64,
}

impl InputSummary {
    fn new(source_iso: PathBuf) -> Self {
        Self {
            source_iso,
            files_scanned: 0,
            candidates_found: 0,
            extracted: 0,
            would_extract: 0,
            skipped: 0,
            failed: 0,
        }
    }

    fn record(&mut self, status: &ExtractionStatus) {
        match status {
            ExtractionStatus::Extracted | ExtractionStatus::Converted => self.extracted += 1,
            ExtractionStatus::WouldExtract => self.would_extract += 1,
            status if status.is_skipped() => self.skipped += 1,
            status if status.is_failed() => self.failed += 1,
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub dry_run: bool,
    pub inputs: Vec<InputSummary>,
    pub files_scanned: u64,
    pub candidates_found: u64,
    pub extracted: u64,
    pub would_extract: u64,
    pub skipped: u64,
    pub failed: u64,
    pub results: Vec<ExtractionResult>,
}

impl RunSummary {
    fn new(dry_run: bool) -> Self {
        Self {
            dry_run,
            inputs: Vec::new(),
            files_scanned: 0,
            candidates_found: 0,
            extracted: 0,
            would_extract: 0,
            skipped: 0,
            failed: 0,
            results: Vec::new(),
        }
    }

    fn add_input(&mut self, input: InputSummary) {
        self.files_scanned += input.files_scanned;
        self.candidates_found += input.candidates_found;
        self.extracted += input.extracted;
        self.would_extract += input.would_extract;
        self.skipped += input.skipped;
        self.failed += input.failed;
        self.inputs.push(input);
    }
}

pub fn extract_jpegs(config: Config) -> AppResult<RunSummary> {
    if !config.dry_run {
        fs::create_dir_all(&config.output_dir).map_err(|err| {
            AppError::io(
                format!(
                    "could not create output directory {}",
                    config.output_dir.display()
                ),
                err,
            )
        })?;
    }

    let mut run = RunSummary::new(config.dry_run);

    for input_path in &config.inputs {
        let mut iso = IsoImage::open(input_path)?;
        let entries = iso.entries()?;
        let mut input = InputSummary::new(input_path.clone());

        for entry in entries
            .iter()
            .filter(|entry| entry.kind == IsoEntryKind::File)
        {
            input.files_scanned += 1;

            if !jpeg::has_allowed_extension(&entry.path, &config.extensions) {
                continue;
            }

            input.candidates_found += 1;
            let result = extract_one(&config, input_path, &mut iso, entry);
            input.record(&result.status);
            run.results.push(result);
        }

        run.add_input(input);
    }

    if let Some(path) = &config.manifest_path {
        manifest::write_manifest(path, &run)?;
    }

    Ok(run)
}

fn extract_one(
    config: &Config,
    source_iso: &Path,
    iso: &mut IsoImage,
    entry: &IsoEntry,
) -> ExtractionResult {
    let output_path = match paths::output_path(&config.output_dir, source_iso, &entry.path) {
        Ok(path) => path,
        Err(error) => {
            return ExtractionResult {
                source_iso: source_iso.to_path_buf(),
                internal_path: entry.path.clone(),
                output_path: None,
                status: ExtractionStatus::SkippedInvalidPath,
                size: entry.size,
                error: Some(error),
            };
        }
    };

    if config.dry_run {
        return ExtractionResult {
            source_iso: source_iso.to_path_buf(),
            internal_path: entry.path.clone(),
            output_path: Some(output_path),
            status: ExtractionStatus::WouldExtract,
            size: entry.size,
            error: None,
        };
    }

    if output_path.exists() && !config.overwrite {
        return ExtractionResult {
            source_iso: source_iso.to_path_buf(),
            internal_path: entry.path.clone(),
            output_path: Some(output_path),
            status: ExtractionStatus::SkippedExisting,
            size: entry.size,
            error: Some("output file already exists".into()),
        };
    }

    let bytes = match iso.read_file(entry) {
        Ok(bytes) => bytes,
        Err(error) => {
            return ExtractionResult {
                source_iso: source_iso.to_path_buf(),
                internal_path: entry.path.clone(),
                output_path: Some(output_path),
                status: ExtractionStatus::FailedRead,
                size: entry.size,
                error: Some(error.to_string()),
            };
        }
    };

    if config.validate && jpeg::is_jpeg_name(&entry.path) && !jpeg::has_jpeg_magic(&bytes) {
        return ExtractionResult {
            source_iso: source_iso.to_path_buf(),
            internal_path: entry.path.clone(),
            output_path: Some(output_path),
            status: ExtractionStatus::FailedValidation,
            size: entry.size,
            error: Some("missing JPEG start marker FF D8".into()),
        };
    }

    match write_atomic(&output_path, &bytes, config.overwrite) {
        Ok(()) => {
            if config.convert_to_jpg && !jpeg::is_jpeg_name(&entry.path) {
                return convert_extracted_file(source_iso, entry, &output_path, config.overwrite);
            }

            ExtractionResult {
                source_iso: source_iso.to_path_buf(),
                internal_path: entry.path.clone(),
                output_path: Some(output_path),
                status: ExtractionStatus::Extracted,
                size: entry.size,
                error: None,
            }
        }
        Err(error) => ExtractionResult {
            source_iso: source_iso.to_path_buf(),
            internal_path: entry.path.clone(),
            output_path: Some(output_path),
            status: ExtractionStatus::FailedWrite,
            size: entry.size,
            error: Some(error.to_string()),
        },
    }
}

fn convert_extracted_file(
    source_iso: &Path,
    entry: &IsoEntry,
    extracted_path: &Path,
    overwrite: bool,
) -> ExtractionResult {
    let jpg_path = extracted_path.with_extension("jpg");

    if jpg_path.exists() && !overwrite {
        return ExtractionResult {
            source_iso: source_iso.to_path_buf(),
            internal_path: entry.path.clone(),
            output_path: Some(jpg_path),
            status: ExtractionStatus::SkippedExisting,
            size: entry.size,
            error: Some("converted JPG already exists".into()),
        };
    }

    let status = Command::new("magick")
        .arg(extracted_path)
        .arg(&jpg_path)
        .status();

    match status {
        Ok(status) if status.success() => ExtractionResult {
            source_iso: source_iso.to_path_buf(),
            internal_path: entry.path.clone(),
            output_path: Some(jpg_path),
            status: ExtractionStatus::Converted,
            size: entry.size,
            error: None,
        },
        Ok(status) => ExtractionResult {
            source_iso: source_iso.to_path_buf(),
            internal_path: entry.path.clone(),
            output_path: Some(jpg_path),
            status: ExtractionStatus::FailedConversion,
            size: entry.size,
            error: Some(format!("magick exited with status {status}")),
        },
        Err(error) => ExtractionResult {
            source_iso: source_iso.to_path_buf(),
            internal_path: entry.path.clone(),
            output_path: Some(jpg_path),
            status: ExtractionStatus::FailedConversion,
            size: entry.size,
            error: Some(format!("could not run magick: {error}")),
        },
    }
}

fn write_atomic(path: &Path, bytes: &[u8], overwrite: bool) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "output path has no parent"))?;
    fs::create_dir_all(parent)?;

    let temp_path = temp_path_for(path)?;
    let write_result = (|| {
        let mut temp_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        temp_file.write_all(bytes)?;
        temp_file.flush()?;

        if !overwrite && path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "output file already exists",
            ));
        }

        fs::rename(&temp_path, path)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result
}

fn temp_path_for(path: &Path) -> io::Result<PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid output filename"))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(path.with_file_name(format!(".{file_name}.tmp-{}-{nonce}", std::process::id())))
}
