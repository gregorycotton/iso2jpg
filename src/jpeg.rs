pub fn is_jpeg_name(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".jpg") || lower.ends_with(".jpeg")
}

pub fn has_allowed_extension(path: &str, extensions: &[String]) -> bool {
    let lower = path.to_ascii_lowercase();
    extensions.iter().any(|extension| {
        lower
            .strip_suffix(extension)
            .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

pub fn has_jpeg_magic(bytes: &[u8]) -> bool {
    matches!(bytes, [0xff, 0xd8, ..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_jpeg_extensions_case_insensitively() {
        assert!(is_jpeg_name("photo.jpg"));
        assert!(is_jpeg_name("photo.jpeg"));
        assert!(is_jpeg_name("photo.JPG"));
        assert!(is_jpeg_name("photo.JPEG"));
        assert!(!is_jpeg_name("photo.png"));
        assert!(!is_jpeg_name("photo.jpg.exe"));
    }

    #[test]
    fn detects_configured_extensions() {
        let extensions = vec!["pcd".to_string(), "bmp".to_string()];
        assert!(has_allowed_extension("1000.PCD", &extensions));
        assert!(has_allowed_extension("folder/cover.bmp", &extensions));
        assert!(!has_allowed_extension("photo.jpg", &extensions));
        assert!(!has_allowed_extension("notpcd", &extensions));
    }

    #[test]
    fn checks_jpeg_magic() {
        assert!(has_jpeg_magic(&[0xff, 0xd8, 0xff, 0xe0]));
        assert!(!has_jpeg_magic(&[0xff]));
        assert!(!has_jpeg_magic(&[0x89, b'P', b'N', b'G']));
    }
}
