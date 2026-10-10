use std::env;
use std::fs;
use std::path::Path;
use std::process::exit;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("qpdf: no arguments provided");
        exit(2);
    }

    // 1. Handle --show-npages <file>
    if args[0] == "--show-npages" {
        if args.len() < 2 {
            eprintln!("qpdf: --show-npages requires a file argument");
            exit(2);
        }
        let file_path = &args[1];
        let content = match fs::read(file_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("qpdf: cannot read {}: {}", file_path, e);
                exit(2);
            }
        };

        let content_str = String::from_utf8_lossy(&content);
        if content_str.contains("__MOCK_QPDF_FAIL__") {
            eprintln!("qpdf: {}: damaged or unrecoverable PDF", file_path);
            exit(2);
        }
        if content_str.contains("__MOCK_QPDF_PASSWORD__") {
            eprintln!("qpdf: {}: operation requires a password", file_path);
            exit(2);
        }

        let pages = extract_page_count(&content_str);
        println!("{}", pages);
        exit(0);
    }

    // 2. Handle --empty --pages ... -- output.pdf
    if args[0] == "--empty" && args.len() >= 4 && args[1] == "--pages" {
        let dash_dash_pos = match args.iter().position(|a| a == "--") {
            Some(p) => p,
            None => {
                eprintln!("qpdf: missing '--' separator before output file");
                exit(2);
            }
        };

        if dash_dash_pos + 1 >= args.len() {
            eprintln!("qpdf: missing output file after '--'");
            exit(2);
        }

        let output_path = &args[dash_dash_pos + 1];
        let page_args = &args[2..dash_dash_pos];

        let mut total_pages = 0;
        let mut i = 0;
        while i < page_args.len() {
            let input_file = &page_args[i];
            let content = match fs::read(input_file) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("qpdf: cannot read input file {}: {}", input_file, e);
                    exit(2);
                }
            };
            let content_str = String::from_utf8_lossy(&content);
            if content_str.contains("__MOCK_QPDF_FAIL__") {
                eprintln!("qpdf: error processing {}", input_file);
                exit(2);
            }

            let file_pages = extract_page_count(&content_str);

            // Check if next arg is a page range
            if i + 1 < page_args.len() && !Path::new(&page_args[i + 1]).exists() {
                let range = &page_args[i + 1];
                let count = parse_range_count(range, file_pages);
                total_pages += count;
                i += 2;
            } else {
                total_pages += file_pages;
                i += 1;
            }
        }

        let output_bytes = generate_pdf(total_pages);
        if let Err(e) = fs::write(output_path, output_bytes) {
            eprintln!("qpdf: failed to write {}: {}", output_path, e);
            exit(2);
        }
        exit(0);
    }

    eprintln!("qpdf: unsupported mock arguments: {:?}", args);
    exit(2);
}

fn extract_page_count(content: &str) -> u32 {
    if let Some(pos) = content.find("/Count ") {
        let rest = &content[pos + 7..];
        let num_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = num_str.parse::<u32>() {
            return n.max(1);
        }
    }
    1
}

fn parse_range_count(range: &str, max_pages: u32) -> u32 {
    let mut count = 0;
    for part in range.split(',') {
        let part = part.trim();
        if let Some((start, end)) = part.split_once('-') {
            let s: u32 = start.trim().parse().unwrap_or(1);
            let e: u32 = if end.trim() == "z" {
                max_pages
            } else {
                end.trim().parse().unwrap_or(max_pages)
            };
            if e >= s {
                count += (e - s) + 1;
            }
        } else if part == "z" || part.parse::<u32>().is_ok() {
            count += 1;
        }
    }
    count.max(1)
}

fn generate_pdf(page_count: u32) -> Vec<u8> {
    format!(
        "%PDF-1.4\n\
        1 0 obj\n\
        << /Type /Catalog /Pages 2 0 R >>\n\
        endobj\n\
        2 0 obj\n\
        << /Type /Pages /Count {} >>\n\
        endobj\n\
        xref\n\
        0 3\n\
        0000000000 65535 f \n\
        0000000009 00000 n \n\
        0000000058 00000 n \n\
        trailer\n\
        << /Size 3 /Root 1 0 R >>\n\
        startxref\n\
        120\n\
        %%EOF\n",
        page_count
    )
    .into_bytes()
}
