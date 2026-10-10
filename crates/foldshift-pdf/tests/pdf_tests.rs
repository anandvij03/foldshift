use std::sync::Mutex;

use foldshift_core::FcError;
use foldshift_pdf::{ExtractOptions, MergeOptions, extract, inspect, merge};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn create_test_pdf(pages: u32) -> Vec<u8> {
    format!(
        "%PDF-1.7\n\
        1 0 obj\n\
        << /Type /Catalog /Pages 2 0 R >>\n\
        endobj\n\
        2 0 obj\n\
        << /Type /Pages /Count {} >>\n\
        endobj\n\
        xref\n\
        0 3\n\
        0000000000 65535 f \n\
        trailer\n\
        << /Size 3 /Root 1 0 R >>\n\
        startxref\n\
        100\n\
        %%EOF\n",
        pages
    )
    .into_bytes()
}

#[test]
fn same_file_and_force_policy_with_mock_engine() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock_exe = env!("CARGO_BIN_EXE_mock-qpdf");
    unsafe { std::env::set_var("FOLDSHIFT_QPDF_PATH", mock_exe) };

    let dir = tempfile::tempdir().unwrap();
    let in1 = dir.path().join("in1.pdf");
    let in2 = dir.path().join("in2.pdf");
    let out = dir.path().join("out.pdf");

    std::fs::write(&in1, create_test_pdf(2)).unwrap();
    std::fs::write(&in2, create_test_pdf(3)).unwrap();

    // 1. Initial merge succeeds
    merge(
        &[in1.clone(), in2.clone()],
        &out,
        MergeOptions { force: false },
    )
    .unwrap();
    assert!(out.exists());

    let insp = inspect(&out).unwrap();
    assert_eq!(insp.properties.get("page_count").unwrap(), &5);

    // 2. Overwrite without force fails with OutputExists
    let err = merge(
        &[in1.clone(), in2.clone()],
        &out,
        MergeOptions { force: false },
    )
    .unwrap_err();
    assert!(matches!(err, FcError::OutputExists(_)));

    // 3. Overwrite with force succeeds
    merge(
        &[in1.clone(), in2.clone()],
        &out,
        MergeOptions { force: true },
    )
    .unwrap();

    // 4. In-place merge over input with force preserves file during processing
    let initial_in1_bytes = std::fs::read(&in1).unwrap();
    merge(
        &[in1.clone(), in2.clone()],
        &in1,
        MergeOptions { force: true },
    )
    .unwrap();
    let updated_in1_bytes = std::fs::read(&in1).unwrap();
    assert_ne!(initial_in1_bytes, updated_in1_bytes);

    // 5. Page extraction
    let extract_out = dir.path().join("extracted.pdf");
    extract(
        &out,
        &extract_out,
        ExtractOptions {
            force: false,
            pages: "1-2".into(),
        },
    )
    .unwrap();
    assert!(extract_out.exists());
    let extract_insp = inspect(&extract_out).unwrap();
    assert_eq!(extract_insp.properties.get("page_count").unwrap(), &2);

    unsafe { std::env::remove_var("FOLDSHIFT_QPDF_PATH") };
}

#[test]
fn failed_qpdf_preserves_original_file() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock_exe = env!("CARGO_BIN_EXE_mock-qpdf");
    unsafe { std::env::set_var("FOLDSHIFT_QPDF_PATH", mock_exe) };

    let dir = tempfile::tempdir().unwrap();
    let corrupt_in = dir.path().join("corrupt.pdf");
    std::fs::write(&corrupt_in, b"%PDF-1.4\n__MOCK_QPDF_FAIL__\n%%EOF\n").unwrap();

    let out = dir.path().join("out.pdf");
    std::fs::write(&out, b"pre-existing-good-data").unwrap();

    // Merge should fail because of corrupt input
    let err = merge(
        std::slice::from_ref(&corrupt_in),
        &out,
        MergeOptions { force: true },
    )
    .unwrap_err();
    assert!(matches!(err, FcError::Conversion(_)));

    // Pre-existing good output was NOT modified or truncated!
    assert_eq!(std::fs::read(&out).unwrap(), b"pre-existing-good-data");

    unsafe { std::env::remove_var("FOLDSHIFT_QPDF_PATH") };
}
