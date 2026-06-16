use crate::errors::{AppError, AppResult};
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const SECTOR_SIZE: usize = 2048;
const VOLUME_DESCRIPTOR_START: u64 = 16;
const MAX_VOLUME_DESCRIPTORS: u64 = 256;
const MAX_DIRECTORIES: usize = 1_000_000;

#[derive(Debug)]
pub struct IsoImage {
    file: File,
    root: DirectoryRef,
    joliet: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsoEntry {
    pub path: String,
    pub size: u64,
    pub kind: IsoEntryKind,
    pub(crate) extent: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsoEntryKind {
    File,
    Directory,
    Other,
}

#[derive(Debug, Clone, Copy)]
struct DirectoryRef {
    extent: u32,
    size: u32,
}

#[derive(Debug, Clone)]
struct ParsedRecord {
    identifier: RecordIdentifier,
    extent: u32,
    size: u32,
    kind: IsoEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RecordIdentifier {
    Current,
    Parent,
    Name(String),
}

impl IsoImage {
    pub fn open(path: &Path) -> AppResult<Self> {
        let mut file = File::open(path)
            .map_err(|err| AppError::io(format!("could not open ISO {}", path.display()), err))?;

        let mut primary_root = None;
        let mut joliet_root = None;

        for descriptor_index in 0..MAX_VOLUME_DESCRIPTORS {
            let sector = VOLUME_DESCRIPTOR_START + descriptor_index;
            let mut descriptor = [0_u8; SECTOR_SIZE];
            file.seek(SeekFrom::Start(sector * SECTOR_SIZE as u64))
                .map_err(|err| AppError::io("could not seek ISO volume descriptor", err))?;
            file.read_exact(&mut descriptor)
                .map_err(|err| AppError::io("could not read ISO volume descriptor", err))?;

            if &descriptor[1..6] != b"CD001" {
                if descriptor_index == 0 {
                    return Err(AppError::iso(
                        "not an ISO 9660 image: missing CD001 descriptor",
                    ));
                }
                continue;
            }

            match descriptor[0] {
                1 => {
                    let root = parse_root_directory(&descriptor, false)?;
                    primary_root = Some(root);
                }
                2 if is_joliet_descriptor(&descriptor) => {
                    let root = parse_root_directory(&descriptor, true)?;
                    joliet_root = Some(root);
                }
                255 => break,
                _ => {}
            }
        }

        if let Some(root) = joliet_root {
            Ok(Self {
                file,
                root,
                joliet: true,
            })
        } else if let Some(root) = primary_root {
            Ok(Self {
                file,
                root,
                joliet: false,
            })
        } else {
            Err(AppError::iso("ISO has no primary volume descriptor"))
        }
    }

    pub fn entries(&mut self) -> AppResult<Vec<IsoEntry>> {
        let mut entries = Vec::new();
        let mut stack = vec![(Vec::<String>::new(), self.root)];
        let mut visited = HashSet::new();

        while let Some((prefix, directory)) = stack.pop() {
            if !visited.insert((directory.extent, directory.size)) {
                continue;
            }

            if visited.len() > MAX_DIRECTORIES {
                return Err(AppError::iso("ISO directory limit exceeded"));
            }

            let data = self.read_extent(directory.extent, directory.size as u64)?;
            for record in parse_directory_records(&data, self.joliet)? {
                let RecordIdentifier::Name(name) = record.identifier else {
                    continue;
                };

                let mut path_parts = prefix.clone();
                path_parts.push(name);
                let path = path_parts.join("/");

                entries.push(IsoEntry {
                    path: path.clone(),
                    size: record.size as u64,
                    kind: record.kind,
                    extent: record.extent,
                });

                if record.kind == IsoEntryKind::Directory {
                    stack.push((
                        path_parts,
                        DirectoryRef {
                            extent: record.extent,
                            size: record.size,
                        },
                    ));
                }
            }
        }

        Ok(entries)
    }

    pub fn read_file(&mut self, entry: &IsoEntry) -> AppResult<Vec<u8>> {
        if entry.kind != IsoEntryKind::File {
            return Err(AppError::iso(format!(
                "{} is not a regular ISO file",
                entry.path
            )));
        }

        self.read_extent(entry.extent, entry.size)
    }

    fn read_extent(&mut self, extent: u32, size: u64) -> AppResult<Vec<u8>> {
        let offset = (extent as u64)
            .checked_mul(SECTOR_SIZE as u64)
            .ok_or_else(|| AppError::iso("ISO extent offset overflow"))?;
        let size = usize::try_from(size).map_err(|_| AppError::iso("ISO file is too large"))?;
        let mut data = vec![0_u8; size];

        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|err| AppError::io("could not seek ISO extent", err))?;
        self.file
            .read_exact(&mut data)
            .map_err(|err| AppError::io("could not read ISO extent", err))?;

        Ok(data)
    }
}

fn parse_root_directory(descriptor: &[u8], joliet: bool) -> AppResult<DirectoryRef> {
    let record = parse_directory_record(&descriptor[156..], joliet)?;
    Ok(DirectoryRef {
        extent: record.extent,
        size: record.size,
    })
}

fn parse_directory_records(data: &[u8], joliet: bool) -> AppResult<Vec<ParsedRecord>> {
    let mut records = Vec::new();
    let mut offset = 0;

    while offset < data.len() {
        let length = data[offset] as usize;
        if length == 0 {
            offset = next_sector_boundary(offset);
            continue;
        }

        let end = offset
            .checked_add(length)
            .ok_or_else(|| AppError::iso("directory record offset overflow"))?;
        if end > data.len() {
            return Err(AppError::iso(
                "directory record extends past directory data",
            ));
        }

        records.push(parse_directory_record(&data[offset..end], joliet)?);
        offset = end;
    }

    Ok(records)
}

fn parse_directory_record(record: &[u8], joliet: bool) -> AppResult<ParsedRecord> {
    if record.len() < 34 {
        return Err(AppError::iso("directory record is too short"));
    }

    let identifier_len = record[32] as usize;
    let identifier_start: usize = 33;
    let identifier_end = identifier_start
        .checked_add(identifier_len)
        .ok_or_else(|| AppError::iso("directory identifier length overflow"))?;
    if identifier_end > record.len() {
        return Err(AppError::iso("directory identifier extends past record"));
    }

    let flags = record[25];
    let kind = if flags & 0x02 != 0 {
        IsoEntryKind::Directory
    } else if flags & 0x01 != 0 {
        IsoEntryKind::Other
    } else {
        IsoEntryKind::File
    };

    Ok(ParsedRecord {
        identifier: decode_identifier(&record[identifier_start..identifier_end], joliet),
        extent: read_u32_le(record, 2)?,
        size: read_u32_le(record, 10)?,
        kind,
    })
}

fn decode_identifier(bytes: &[u8], joliet: bool) -> RecordIdentifier {
    if bytes == [0] {
        return RecordIdentifier::Current;
    }
    if bytes == [1] {
        return RecordIdentifier::Parent;
    }

    let decoded = if joliet && bytes.len() % 2 == 0 {
        let units = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    };

    RecordIdentifier::Name(strip_iso_version(decoded))
}

fn strip_iso_version(mut name: String) -> String {
    if let Some(version_start) = name.rfind(';') {
        if name[version_start + 1..]
            .chars()
            .all(|ch| ch.is_ascii_digit())
        {
            name.truncate(version_start);
        }
    }

    if name.ends_with('.') {
        name.pop();
    }

    name
}

fn read_u32_le(record: &[u8], offset: usize) -> AppResult<u32> {
    let bytes = record
        .get(offset..offset + 4)
        .ok_or_else(|| AppError::iso("directory record is missing a u32 field"))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn is_joliet_descriptor(descriptor: &[u8]) -> bool {
    matches!(&descriptor[88..91], b"%/@" | b"%/C" | b"%/E")
}

fn next_sector_boundary(offset: usize) -> usize {
    ((offset / SECTOR_SIZE) + 1) * SECTOR_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_iso_version_suffix() {
        assert_eq!(strip_iso_version("PHOTO.JPG;1".to_string()), "PHOTO.JPG");
        assert_eq!(strip_iso_version("FOLDER.".to_string()), "FOLDER");
        assert_eq!(strip_iso_version("A;B".to_string()), "A;B");
    }
}
