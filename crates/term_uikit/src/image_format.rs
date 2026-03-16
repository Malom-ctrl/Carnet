pub struct ImageFormat {
    pub mime: &'static str,
    pub matches: fn(&[u8]) -> bool,
}

pub const SUPPORTED_IMAGE_FORMATS: &[ImageFormat] = &[
    ImageFormat {
        mime: "image/png",
        matches: |d| d.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
    },
    ImageFormat {
        mime: "image/jpeg",
        matches: |d| d.starts_with(&[0xFF, 0xD8, 0xFF]),
    },
    ImageFormat {
        mime: "image/webp",
        matches: |d| d.starts_with(b"RIFF") && d.len() > 8 && &d[8..12] == b"WEBP",
    },
    ImageFormat {
        mime: "image/gif",
        matches: |d| d.starts_with(b"GIF8"),
    },
    ImageFormat {
        mime: "image/svg+xml",
        matches: |d| d.starts_with(b"<?xml") || d.starts_with(b"<svg"),
    },
];

pub fn detect_mime(data: &[u8]) -> &'static str {
    for format in SUPPORTED_IMAGE_FORMATS {
        if (format.matches)(data) {
            return format.mime;
        }
    }
    "application/octet-stream"
}
