use pdf_oxide::PdfDocument;
use std::env;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

const MAX_PDF_BYTES: u64 = 100 * 1024 * 1024;

struct ProbeResult {
    pages: usize,
    text_pages: usize,
    text_chars: usize,
    spans: usize,
    elapsed: Duration,
    preview: String,
}

fn main() -> ExitCode {
    let paths = env::args().skip(1).collect::<Vec<_>>();
    if paths.is_empty() {
        eprintln!(
            "Usage: cargo run --manifest-path scripts/pdf-extraction-spike/Cargo.toml -- <file.pdf>..."
        );
        return ExitCode::FAILURE;
    }

    let mut failed = false;
    for path in paths {
        match probe_pdf(Path::new(&path)) {
            Ok(result) => {
                println!(
                    "{}\n  pages: {}\n  pages with text: {}\n  text characters: {}\n  spans: {}\n  elapsed: {:.2?}\n  preview: {}",
                    path,
                    result.pages,
                    result.text_pages,
                    result.text_chars,
                    result.spans,
                    result.elapsed,
                    result.preview
                );
            }
            Err(error) => {
                failed = true;
                eprintln!("{path}\n  error: {error}");
            }
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Exercise the exact text and coordinate APIs Papercut would use without
/// committing the production page-layer schema before the spike passes.
fn probe_pdf(path: &Path) -> Result<ProbeResult, String> {
    let bytes = path
        .metadata()
        .map_err(|error| format!("Failed to inspect PDF: {error}"))?
        .len();
    if bytes == 0 || bytes > MAX_PDF_BYTES {
        return Err(format!(
            "PDF must be between 1 byte and {} MB",
            MAX_PDF_BYTES / 1024 / 1024
        ));
    }

    let started = Instant::now();
    let document =
        PdfDocument::open(path).map_err(|error| format!("Failed to open PDF: {error}"))?;
    let pages = document
        .page_count()
        .map_err(|error| format!("Failed to count PDF pages: {error}"))?;
    let mut result = ProbeResult {
        pages,
        text_pages: 0,
        text_chars: 0,
        spans: 0,
        elapsed: Duration::ZERO,
        preview: String::new(),
    };

    for page in 0..pages {
        let text = document
            .extract_text(page)
            .map_err(|error| format!("Failed to extract page {} text: {error}", page + 1))?;
        let spans = document
            .extract_spans(page)
            .map_err(|error| format!("Failed to extract page {} spans: {error}", page + 1))?;
        if spans.iter().any(|span| {
            let bounds = span.bbox;
            !bounds.x.is_finite()
                || !bounds.y.is_finite()
                || !bounds.width.is_finite()
                || !bounds.height.is_finite()
        }) {
            return Err(format!("Page {} contains non-finite text bounds", page + 1));
        }

        let text_chars = text.chars().count();
        if text_chars > 0 {
            result.text_pages += 1;
            if result.preview.is_empty() {
                result.preview = text
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .chars()
                    .take(160)
                    .collect();
            }
        }
        result.text_chars += text_chars;
        result.spans += spans.len();
    }

    result.elapsed = started.elapsed();
    Ok(result)
}
