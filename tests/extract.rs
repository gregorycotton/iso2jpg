use iso_extract_jpegs::{Config, extract_jpegs};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SECTOR_SIZE: usize = 2048;

#[test]
fn extracts_jpeg_from_minimal_iso() {
    let root = unique_temp_dir("iso_extract_jpegs_extract");
    let iso_path = root.join("disc.iso");
    let output_dir = root.join("out");
    let manifest_path = root.join("manifest.json");
    let jpeg = [0xff, 0xd8, 0xff, 0xe0, b't', b'e', b's', b't', 0xff, 0xd9];

    write_minimal_iso(&iso_path, "PHOTO.JPG;1", &jpeg);

    let summary = extract_jpegs(Config {
        inputs: vec![iso_path.clone()],
        output_dir: output_dir.clone(),
        extensions: vec!["jpg".to_string(), "jpeg".to_string()],
        dry_run: false,
        validate: true,
        overwrite: false,
        convert_to_jpg: false,
        manifest_path: Some(manifest_path.clone()),
        verbose: false,
    })
    .unwrap();

    assert_eq!(summary.files_scanned, 1);
    assert_eq!(summary.candidates_found, 1);
    assert_eq!(summary.extracted, 1);
    assert_eq!(summary.failed, 0);

    let extracted = output_dir.join("disc").join("PHOTO.JPG");
    assert_eq!(fs::read(extracted).unwrap(), jpeg);
    assert!(
        fs::read_to_string(manifest_path)
            .unwrap()
            .contains("\"status\": \"extracted\"")
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn dry_run_does_not_write_files() {
    let root = unique_temp_dir("iso_extract_jpegs_dry_run");
    let iso_path = root.join("disc.iso");
    let output_dir = root.join("out");
    let jpeg = [0xff, 0xd8, 0xff, 0xe0];

    write_minimal_iso(&iso_path, "PHOTO.JPG;1", &jpeg);

    let summary = extract_jpegs(Config {
        inputs: vec![iso_path],
        output_dir: output_dir.clone(),
        extensions: vec!["jpg".to_string(), "jpeg".to_string()],
        dry_run: true,
        validate: false,
        overwrite: false,
        convert_to_jpg: false,
        manifest_path: None,
        verbose: false,
    })
    .unwrap();

    assert_eq!(summary.would_extract, 1);
    assert!(!output_dir.exists());

    let _ = fs::remove_dir_all(root);
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_minimal_iso(path: &Path, file_name: &str, file_bytes: &[u8]) {
    let mut iso = vec![0_u8; SECTOR_SIZE * 22];

    write_primary_volume_descriptor(&mut iso);
    write_volume_descriptor_terminator(&mut iso);
    write_root_directory(&mut iso, file_name, file_bytes.len() as u32);

    let file_start = 21 * SECTOR_SIZE;
    iso[file_start..file_start + file_bytes.len()].copy_from_slice(file_bytes);

    fs::write(path, iso).unwrap();
}

fn write_primary_volume_descriptor(iso: &mut [u8]) {
    let sector = &mut iso[16 * SECTOR_SIZE..17 * SECTOR_SIZE];
    sector[0] = 1;
    sector[1..6].copy_from_slice(b"CD001");
    sector[6] = 1;
    write_u16_both(sector, 128, SECTOR_SIZE as u16);
    let root = directory_record(20, SECTOR_SIZE as u32, 0x02, &[0]);
    sector[156..156 + root.len()].copy_from_slice(&root);
}

fn write_volume_descriptor_terminator(iso: &mut [u8]) {
    let sector = &mut iso[17 * SECTOR_SIZE..18 * SECTOR_SIZE];
    sector[0] = 255;
    sector[1..6].copy_from_slice(b"CD001");
    sector[6] = 1;
}

fn write_root_directory(iso: &mut [u8], file_name: &str, file_size: u32) {
    let sector = &mut iso[20 * SECTOR_SIZE..21 * SECTOR_SIZE];
    let mut offset = 0;

    for record in [
        directory_record(20, SECTOR_SIZE as u32, 0x02, &[0]),
        directory_record(20, SECTOR_SIZE as u32, 0x02, &[1]),
        directory_record(21, file_size, 0x00, file_name.as_bytes()),
    ] {
        sector[offset..offset + record.len()].copy_from_slice(&record);
        offset += record.len();
    }
}

fn directory_record(extent: u32, size: u32, flags: u8, identifier: &[u8]) -> Vec<u8> {
    let padding = if identifier.len() % 2 == 0 { 1 } else { 0 };
    let length = 33 + identifier.len() + padding;
    let mut record = vec![0_u8; length];
    record[0] = length as u8;
    write_u32_both(&mut record, 2, extent);
    write_u32_both(&mut record, 10, size);
    record[25] = flags;
    write_u16_both(&mut record, 28, 1);
    record[32] = identifier.len() as u8;
    record[33..33 + identifier.len()].copy_from_slice(identifier);
    record
}

fn write_u16_both(target: &mut [u8], offset: usize, value: u16) {
    target[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    target[offset + 2..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn write_u32_both(target: &mut [u8], offset: usize, value: u32) {
    target[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    target[offset + 4..offset + 8].copy_from_slice(&value.to_be_bytes());
}
