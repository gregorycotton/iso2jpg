use std::path::{Component, Path, PathBuf};

pub fn output_path(
    output_dir: &Path,
    source_iso: &Path,
    internal_path: &str,
) -> Result<PathBuf, String> {
    let iso_name = source_iso
        .file_stem()
        .or_else(|| source_iso.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("iso");

    let iso_dir = sanitize_component(iso_name)?;
    let parts = sanitize_internal_path(internal_path)?;

    let mut path = output_dir.to_path_buf();
    path.push(iso_dir);
    for part in parts {
        path.push(part);
    }
    Ok(path)
}

pub fn sanitize_internal_path(path: &str) -> Result<Vec<String>, String> {
    if path.is_empty() {
        return Err("empty internal path".into());
    }

    if path.starts_with('/') || path.starts_with('\\') {
        return Err("absolute internal path rejected".into());
    }

    if looks_like_windows_drive_path(path) {
        return Err("Windows drive-prefixed internal path rejected".into());
    }

    let mut components = Vec::new();
    for raw in path.split(['/', '\\']) {
        if raw.is_empty() || raw == "." {
            continue;
        }
        if raw == ".." {
            return Err("parent directory component rejected".into());
        }
        components.push(sanitize_component(raw)?);
    }

    if components.is_empty() {
        return Err("internal path has no filename".into());
    }

    Ok(components)
}

fn sanitize_component(component: &str) -> Result<String, String> {
    if component.is_empty() {
        return Err("empty path component rejected".into());
    }
    if component == "." || component == ".." {
        return Err("unsafe path component rejected".into());
    }
    if component.contains('\0') {
        return Err("NUL byte in path component rejected".into());
    }
    if Path::new(component).components().any(|part| {
        matches!(
            part,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err("unsafe path component rejected".into());
    }

    let mut clean = String::with_capacity(component.len());
    for ch in component.chars() {
        if ch.is_control() {
            return Err("control character in path component rejected".into());
        }
        if matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*') {
            clean.push('_');
        } else {
            clean.push(ch);
        }
    }

    let clean = clean.trim_matches([' ', '.']).to_string();
    if clean.is_empty() {
        return Err("path component became empty after sanitization".into());
    }

    Ok(clean)
}

fn looks_like_windows_drive_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_path_under_iso_directory() {
        let path = output_path(Path::new("out"), Path::new("disc.iso"), "photos/a.jpg").unwrap();
        assert_eq!(
            path,
            PathBuf::from("out")
                .join("disc")
                .join("photos")
                .join("a.jpg")
        );
    }

    #[test]
    fn rejects_traversal_and_absolute_paths() {
        for bad in [
            "../evil.jpg",
            "../../evil.jpg",
            "/evil.jpg",
            "C:\\evil.jpg",
            "folder/../../../evil.jpg",
            "folder\\..\\evil.jpg",
        ] {
            assert!(sanitize_internal_path(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn sanitizes_windows_reserved_characters() {
        let parts = sanitize_internal_path("folder/a:b?.jpg").unwrap();
        assert_eq!(parts, vec!["folder".to_string(), "a_b_.jpg".to_string()]);
    }
}
