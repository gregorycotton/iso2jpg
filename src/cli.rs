use crate::errors::{AppError, AppResult};
use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub inputs: Vec<PathBuf>,
    pub output_dir: PathBuf,
    pub extensions: Vec<String>,
    pub dry_run: bool,
    pub validate: bool,
    pub overwrite: bool,
    pub convert_to_jpg: bool,
    pub manifest_path: Option<PathBuf>,
    pub verbose: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliAction {
    Run(Config),
    Help,
    Version,
}

pub fn parse_args<I>(args: I) -> AppResult<CliAction>
where
    I: IntoIterator<Item = OsString>,
{
    let mut inputs = Vec::new();
    let mut output_dir = None;
    let mut extensions = vec!["jpg".to_string(), "jpeg".to_string()];
    let mut dry_run = false;
    let mut validate = false;
    let mut overwrite = false;
    let mut convert_to_jpg = false;
    let mut manifest_path = None;
    let mut verbose = false;

    let mut iter = args.into_iter().peekable();
    while let Some(arg) = iter.next() {
        let text = arg.to_string_lossy();
        match text.as_ref() {
            "-h" | "--help" => return Ok(CliAction::Help),
            "--version" => return Ok(CliAction::Version),
            "-n" | "--dry-run" => dry_run = true,
            "--validate" => validate = true,
            "--overwrite" => overwrite = true,
            "--convert-to-jpg" => convert_to_jpg = true,
            "--verbose" => verbose = true,
            "-o" | "--out" => {
                let value = next_value(&mut iter, "--out")?;
                output_dir = Some(PathBuf::from(value));
            }
            "--manifest" => {
                let value = next_value(&mut iter, "--manifest")?;
                manifest_path = Some(PathBuf::from(value));
            }
            "--extensions" => {
                let value = next_value(&mut iter, "--extensions")?;
                extensions = parse_extensions(&value.to_string_lossy())?;
            }
            _ if text.starts_with("--out=") => {
                output_dir = Some(PathBuf::from(text.trim_start_matches("--out=")));
            }
            _ if text.starts_with("--manifest=") => {
                manifest_path = Some(PathBuf::from(text.trim_start_matches("--manifest=")));
            }
            _ if text.starts_with("--extensions=") => {
                extensions = parse_extensions(text.trim_start_matches("--extensions="))?;
            }
            _ if text.starts_with('-') => {
                return Err(AppError::Cli(format!("unknown option: {text}")));
            }
            _ => inputs.push(PathBuf::from(arg)),
        }
    }

    let output_dir =
        output_dir.ok_or_else(|| AppError::Cli("missing required option: --out <DIR>".into()))?;

    if inputs.is_empty() {
        return Err(AppError::Cli("missing input ISO path".into()));
    }

    if dry_run && overwrite {
        return Err(AppError::Cli(
            "--dry-run and --overwrite cannot be used together".into(),
        ));
    }

    Ok(CliAction::Run(Config {
        inputs,
        output_dir,
        extensions,
        dry_run,
        validate,
        overwrite,
        convert_to_jpg,
        manifest_path,
        verbose,
    }))
}

fn next_value<I>(iter: &mut std::iter::Peekable<I>, option: &str) -> AppResult<OsString>
where
    I: Iterator<Item = OsString>,
{
    let Some(value) = iter.next() else {
        return Err(AppError::Cli(format!("missing value for {option}")));
    };

    if value.to_string_lossy().starts_with('-') {
        return Err(AppError::Cli(format!("missing value for {option}")));
    }

    Ok(value)
}

fn parse_extensions(value: &str) -> AppResult<Vec<String>> {
    let mut extensions = Vec::new();

    for raw in value.split(',') {
        let extension = raw.trim().trim_start_matches('.').to_ascii_lowercase();
        if extension.is_empty() {
            continue;
        }
        if !extension
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            return Err(AppError::Cli(format!(
                "invalid extension in --extensions: {raw}"
            )));
        }
        extensions.push(extension);
    }

    if extensions.is_empty() {
        return Err(AppError::Cli(
            "--extensions must include at least one extension".into(),
        ));
    }

    Ok(extensions)
}

pub fn usage() -> &'static str {
    concat!(
        "Usage: iso-extract-jpegs [OPTIONS] <ISO>...\n",
        "\n",
        "Options:\n",
        "  -o, --out <DIR>          Output directory\n",
        "  -n, --dry-run            Scan only; do not write files\n",
        "      --validate           Validate JPEG magic bytes before writing\n",
        "      --overwrite          Replace existing output files\n",
        "      --manifest <PATH>    Write JSON extraction manifest\n",
        "      --extensions <LIST>  Comma-separated extensions; default jpg,jpeg\n",
        "      --convert-to-jpg     Convert extracted non-JPEG files to .jpg with ImageMagick\n",
        "      --verbose            Print per-file details\n",
        "  -h, --help               Print help\n",
        "      --version            Print version\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_command() {
        let action = parse_args(["disc.iso", "--out", "out"].map(OsString::from)).unwrap();
        match action {
            CliAction::Run(config) => {
                assert_eq!(config.inputs, vec![PathBuf::from("disc.iso")]);
                assert_eq!(config.output_dir, PathBuf::from("out"));
                assert_eq!(config.extensions, vec!["jpg", "jpeg"]);
            }
            _ => panic!("expected run action"),
        }
    }

    #[test]
    fn rejects_dry_run_with_overwrite() {
        let err = parse_args(
            ["disc.iso", "--out", "out", "--dry-run", "--overwrite"].map(OsString::from),
        )
        .unwrap_err();
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn parses_custom_extensions() {
        let action = parse_args(
            ["disc.iso", "--out", "out", "--extensions", "pcd,.bmp"].map(OsString::from),
        )
        .unwrap();
        match action {
            CliAction::Run(config) => assert_eq!(config.extensions, vec!["pcd", "bmp"]),
            _ => panic!("expected run action"),
        }
    }
}
