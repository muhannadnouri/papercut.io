//! Archive-only CSS access; URL admission never delegates to a filesystem or network client.

use super::*;
use percent_encoding::percent_decode_str;

/// Admit only canonical manifest CSS paths, after checking decoded URL syntax as well as raw syntax.
pub(super) fn resolve(
    chapter_path: &str,
    href: &str,
    manifest_css: &HashSet<&str>,
) -> Result<String, &'static str> {
    let path = local_path(&paths::archive_base_dir(chapter_path), href)?;
    if !manifest_css.contains(path.as_str()) {
        return Err("unread-stylesheet");
    }
    Ok(path)
}

/// Apply the same URL boundary to OPF members and chapter links before canonicalization loses it.
pub(super) fn local_path(base: &str, href: &str) -> Result<String, &'static str> {
    let decoded = percent_decode_str(href)
        .decode_utf8()
        .map_err(|_| "unread-stylesheet")?;
    if decoded.is_empty()
        || decoded.starts_with('/')
        || decoded
            .chars()
            .any(|c| c.is_control() || matches!(c, ':' | '\\' | '?' | '#'))
    {
        return Err("unread-stylesheet");
    }
    resolve_archive_path(base, href).map_err(|_| "unread-stylesheet")
}

/// Bound declared and actual inflated bytes; the normalizer charges bytes before UTF-8 decoding.
pub(super) fn read<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
    limit: usize,
) -> Result<Vec<u8>, &'static str> {
    let over_budget = if limit == 64 * 1024 {
        "sheet-byte-limit"
    } else {
        "css-byte-limit"
    };
    let mut file = archive.by_name(path).map_err(|_| "unread-stylesheet")?;
    if file.size() > limit as u64 {
        return Err(over_budget);
    }
    let mut bytes = Vec::with_capacity(file.size() as usize);
    file.by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "unread-stylesheet")?;
    if bytes.len() > limit {
        return Err(over_budget);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn css_url_admission_checks_decoded_syntax_before_archive_resolution() {
        assert_eq!(
            local_path("OPS/text", "../styles/book%20style.css").unwrap(),
            "OPS/styles/book style.css"
        );
        for href in [
            "",
            "/style.css",
            "//host/style.css",
            "../../../style.css",
            "https://host/style.css",
            "file:style.css",
            "style.css?x",
            "style.css#x",
            "..\\style.css",
            "%2fstyle.css",
            "%5cstyle.css",
            "%2e%2e/%2e%2e/%2e%2e/style.css",
            "style%3fquery.css",
            "http%3aevil.css",
            "%00style.css",
            "%ff.css",
        ] {
            assert!(local_path("OPS/text", href).is_err(), "{href}");
        }
    }

    #[test]
    fn linked_zip_reader_rejects_declared_bytes_before_inflation_and_never_decodes_them() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("style.css", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&[0xff, 0xfe, 0xfd]).unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(
            read(&mut archive, "style.css", 2).unwrap_err(),
            "css-byte-limit"
        );
        assert_eq!(
            read(&mut archive, "style.css", 3).unwrap(),
            [0xff, 0xfe, 0xfd]
        );
    }

    #[test]
    fn xml_base_in_the_opf_never_certifies_the_wrong_manifest_css_path() {
        for (package_base, manifest_base, item_base) in [
            ("xml:base='elsewhere/'", "", ""),
            ("", "xml:base='elsewhere/'", ""),
            ("", "", "xml:base='elsewhere/'"),
        ] {
            let opf = format!("<package {package_base}><manifest {manifest_base}><item id='css' href='style.css' media-type='text/css' {item_base}/><item id='c' href='chapter.xhtml' media-type='application/xhtml+xml'/></manifest><spine><itemref idref='c'/></spine></package>");
            let package = parse_opf(&opf, "OPS/book.opf", "Book").unwrap();
            assert!(package.css_paths.is_empty());
        }
    }
}
