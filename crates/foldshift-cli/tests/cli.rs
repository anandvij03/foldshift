use std::process::Command;

#[test]
fn cli_capabilities_reflects_enabled_capabilities() {
    let result = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .arg("capabilities")
        .output()
        .unwrap();
    assert!(result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("CORE   installed"));

    let json_result = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["capabilities", "--json"])
        .output()
        .unwrap();
    assert!(json_result.status.success());
    let data: Vec<serde_json::Value> = serde_json::from_slice(&json_result.stdout).unwrap();
    let img_convert = data.iter().find(|op| op["id"] == "image.convert").unwrap();

    if cfg!(feature = "image") {
        assert!(stdout.contains("IMAGE  installed"));
        assert_eq!(img_convert["available"], true);
    } else {
        assert!(stdout.contains("IMAGE  not installed"));
        assert_eq!(img_convert["available"], false);
    }
}

#[cfg(not(feature = "image"))]
#[test]
fn cli_core_only_rejects_image_subcommands() {
    for cmd in ["convert", "image", "inspect"] {
        let result = Command::new(env!("CARGO_BIN_EXE_foldshift"))
            .arg(cmd)
            .output()
            .unwrap();
        assert!(!result.status.success());
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains("unrecognized subcommand") || stderr.contains("error:"),
            "expected error for '{cmd}', got: {stderr}"
        );
    }
}

#[cfg(feature = "image")]
#[test]
fn cli_converts_inspects_and_refuses_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.png");
    let output = dir.path().join("result.webp");
    image::DynamicImage::new_rgb8(16, 10).save(&input).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .arg("convert")
        .arg(&input)
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let before = std::fs::read(&output).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .arg("convert")
        .arg(&input)
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(std::fs::read(&output).unwrap(), before);
    let result = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .arg("inspect")
        .arg(&output)
        .arg("--json")
        .output()
        .unwrap();
    assert!(result.status.success());
    let data: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(data["format"], "WebP");
    assert_eq!(data["properties"]["width"], 16);
    assert_eq!(data["properties"]["height"], 10);
}

#[cfg(feature = "image")]
#[test]
fn invalid_options_return_failure() {
    let result = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["convert", "a.png", "b.jpg", "--quality", "101"])
        .output()
        .unwrap();
    assert!(!result.status.success());
}

#[cfg(feature = "image")]
#[test]
fn cli_rejects_animated_gif_and_preserves_destination_on_force() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("animated.gif");
    let output = dir.path().join("destination.png");
    let mut buffer = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut buffer, 2, 2, &[0, 0, 0, 255, 255, 255]).unwrap();
        let frame1 = gif::Frame::from_indexed_pixels(2, 2, [0, 1, 0, 1], None);
        let frame2 = gif::Frame::from_indexed_pixels(2, 2, [1, 0, 1, 0], None);
        encoder.write_frame(&frame1).unwrap();
        encoder.write_frame(&frame2).unwrap();
    }
    std::fs::write(&input, buffer).unwrap();
    std::fs::write(&output, b"pre-existing content").unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["convert"])
        .arg(&input)
        .arg(&output)
        .arg("--force")
        .output()
        .unwrap();

    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("unsupported content"),
        "unexpected stderr: {stderr}"
    );
    assert_eq!(
        std::fs::read(&output).unwrap(),
        b"pre-existing content",
        "existing output must not be modified"
    );
}

#[cfg(feature = "image")]
#[test]
fn cli_rejects_multipage_tiff() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("multipage.tiff");
    let output = dir.path().join("output.png");
    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut tiff = tiff::encoder::TiffEncoder::new(&mut cursor).unwrap();
        let image1 = [10u8, 20, 30, 40, 50, 60];
        tiff.write_image::<tiff::encoder::colortype::RGB8>(2, 1, &image1)
            .unwrap();
        let image2 = [70u8, 80, 90, 100, 110, 120];
        tiff.write_image::<tiff::encoder::colortype::RGB8>(2, 1, &image2)
            .unwrap();
    }
    std::fs::write(&input, cursor.into_inner()).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["convert"])
        .arg(&input)
        .arg(&output)
        .output()
        .unwrap();

    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("unsupported content"),
        "unexpected stderr: {stderr}"
    );
    assert!(!output.exists(), "output file must not be created");
}

#[cfg(feature = "image")]
#[test]
fn cli_rejects_unsupported_destination_format_without_creating_output() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("source.png");
    let nested_dir = dir.path().join("nonexistent_subdir");
    let output = nested_dir.join("result.invalidext");
    image::DynamicImage::new_rgb8(10, 10).save(&input).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["convert"])
        .arg(&input)
        .arg(&output)
        .output()
        .unwrap();

    assert!(!result.status.success());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("unsupported format"),
        "unexpected stderr: {stderr}"
    );
    assert!(!nested_dir.exists(), "directory must not be created");
}

#[cfg(feature = "image")]
fn create_jpeg_with_exif_orientation(width: u32, height: u32, orientation: u8) -> Vec<u8> {
    let base = image::DynamicImage::new_rgb8(width, height);
    let mut base_jpeg = Vec::new();
    base.write_to(
        &mut std::io::Cursor::new(&mut base_jpeg),
        image::ImageFormat::Jpeg,
    )
    .unwrap();

    #[rustfmt::skip]
    let app1 = [
        0xFF, 0xE1, 0x00, 0x22,
        b'E', b'x', b'i', b'f', 0x00, 0x00,
        b'I', b'I', 0x2A, 0x00, 0x08, 0x00, 0x00, 0x00,
        0x01, 0x00,
        0x12, 0x01, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00,
        orientation, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];

    let mut output = Vec::with_capacity(base_jpeg.len() + app1.len());
    output.extend_from_slice(&base_jpeg[..2]); // SOI
    output.extend_from_slice(&app1);
    output.extend_from_slice(&base_jpeg[2..]);
    output
}

#[cfg(feature = "image")]
#[test]
fn cli_inspects_orientation_and_normalizes_geometry() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("portrait.jpg");
    let output = dir.path().join("upright.png");
    // 16x10 with Orientation 6 (Rotate90)
    let jpeg_bytes = create_jpeg_with_exif_orientation(16, 10, 6);
    std::fs::write(&input, jpeg_bytes).unwrap();

    // Inspect via CLI
    let inspect_res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["inspect", "--json"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(inspect_res.status.success());
    let data: serde_json::Value = serde_json::from_slice(&inspect_res.stdout).unwrap();
    assert_eq!(data["properties"]["orientation"], "Rotate90");
    assert_eq!(data["properties"]["width"], 10);
    assert_eq!(data["properties"]["height"], 16);

    // Convert via CLI
    let convert_res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["convert"])
        .arg(&input)
        .arg(&output)
        .output()
        .unwrap();
    assert!(convert_res.status.success());

    // Inspect converted output
    let inspect_out = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["inspect", "--json"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(inspect_out.status.success());
    let data_out: serde_json::Value = serde_json::from_slice(&inspect_out.stdout).unwrap();
    assert_eq!(data_out["properties"]["width"], 10);
    assert_eq!(data_out["properties"]["height"], 16);
}

#[cfg(feature = "image")]
#[test]
fn cli_rejects_transparency_for_jpeg_and_allows_opaque_alpha() {
    let dir = tempfile::tempdir().unwrap();

    // Transparent PNG
    let trans_input = dir.path().join("trans.png");
    let mut img = image::RgbaImage::from_pixel(4, 4, image::Rgba([50, 100, 150, 255]));
    img.put_pixel(0, 0, image::Rgba([50, 100, 150, 0]));
    image::DynamicImage::ImageRgba8(img)
        .save(&trans_input)
        .unwrap();

    let trans_out = dir.path().join("trans.jpg");
    let res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["convert"])
        .arg(&trans_input)
        .arg(&trans_out)
        .output()
        .unwrap();
    assert!(!res.status.success());
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("JPEG cannot preserve transparency"),
        "unexpected stderr: {stderr}"
    );

    // Opaque RGBA PNG
    let opaque_input = dir.path().join("opaque.png");
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        4,
        4,
        image::Rgba([50, 100, 150, 255]),
    ))
    .save(&opaque_input)
    .unwrap();

    let opaque_out = dir.path().join("opaque.jpg");
    let res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["convert"])
        .arg(&opaque_input)
        .arg(&opaque_out)
        .output()
        .unwrap();
    assert!(res.status.success());
    assert!(opaque_out.exists());
}

#[cfg(feature = "image")]
#[test]
fn cli_same_file_policy_and_alias_preservation() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("source.png");
    image::DynamicImage::new_rgb8(12, 12).save(&file).unwrap();
    let initial_bytes = std::fs::read(&file).unwrap();

    // 1. Without --force, in-place convert fails with OutputExists
    let res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["convert"])
        .arg(&file)
        .arg(&file)
        .output()
        .unwrap();
    assert!(!res.status.success());
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(stderr.contains("output already exists"));
    assert_eq!(std::fs::read(&file).unwrap(), initial_bytes);

    // 2. With --force, in-place resize succeeds
    let res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["image", "resize"])
        .arg(&file)
        .arg(&file)
        .args(["--width", "6", "--force"])
        .output()
        .unwrap();
    assert!(res.status.success());
    let inspect_res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["inspect", "--json"])
        .arg(&file)
        .output()
        .unwrap();
    assert!(inspect_res.status.success());
    let data: serde_json::Value = serde_json::from_slice(&inspect_res.stdout).unwrap();
    assert_eq!(data["properties"]["width"], 6);
    assert_eq!(data["properties"]["height"], 6);
}

#[test]
fn cli_packs_list_shows_planned_engines_and_json_format() {
    let result = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["packs", "list"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(stdout.contains("Available capability packs:"));
    assert!(stdout.contains("PDF    qpdf (QPDF Engine)"));
    assert!(stdout.contains("MEDIA  media (Shared Media Engine)"));
    assert!(stdout.contains("Packs directory:"));

    let json_result = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["packs", "list", "--json"])
        .output()
        .unwrap();
    assert!(json_result.status.success());
    let data: Vec<serde_json::Value> = serde_json::from_slice(&json_result.stdout).unwrap();
    assert_eq!(data.len(), 2);
    let pdf_pack = data.iter().find(|p| p["capability"] == "pdf").unwrap();
    assert_eq!(pdf_pack["id"], "qpdf");
    assert_eq!(pdf_pack["installed"], false);
}

#[test]
fn cli_dynamically_discovers_installed_pack_via_env() {
    let dir = tempfile::tempdir().unwrap();
    let packs_dir = dir.path();

    // Verify baseline before pack installation
    let res_before = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .arg("capabilities")
        .env("FOLDSHIFT_PACKS_DIR", packs_dir)
        .output()
        .unwrap();
    let stdout_before = String::from_utf8_lossy(&res_before.stdout);
    assert!(stdout_before.contains("PDF    not installed"));
    assert!(stdout_before.contains("no   pdf.merge"));

    // Create mock installed pack structure
    let bin_dir = packs_dir.join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let exe_path = bin_dir.join("qpdf.exe");
    std::fs::write(&exe_path, b"mock-qpdf-binary").unwrap();

    let manifest = foldshift_core::pack::PackManifest {
        schema_version: "1.0.0".into(),
        id: "qpdf".into(),
        name: "QPDF Engine".into(),
        version: "11.9.1".into(),
        capability: "pdf".into(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        abi: None,
        core_version_requirement: Some(">=0.1.0".into()),
        entrypoint: std::path::PathBuf::from("bin/qpdf.exe"),
        download_bytes: 1_000,
        unpacked_bytes: 2_000,
        sha256: "0000000000000000000000000000000000000000000000000000000000000000".into(),
        file_inventory: vec![foldshift_core::pack::PackFileEntry {
            path: std::path::PathBuf::from("bin/qpdf.exe"),
            bytes: 1_000,
            sha256: None,
        }],
        license: "Apache-2.0".into(),
        notice: None,
        signature: None,
        trusted_key_id: None,
    };

    let mut state = foldshift_core::pack::PackInstallState::new();
    state.record_install(foldshift_core::pack::InstalledPack {
        id: "qpdf".into(),
        capability: "pdf".into(),
        name: "QPDF Engine".into(),
        version: "11.9.1".into(),
        install_dir: packs_dir.to_path_buf(),
        entrypoint: std::path::PathBuf::from("bin/qpdf.exe"),
        installed_at: "2026-10-10T12:00:00Z".into(),
        manifest,
    });
    state.save_to_dir(packs_dir).unwrap();

    // Verify capabilities discovers the installed and healthy PDF pack
    let res_after = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .arg("capabilities")
        .env("FOLDSHIFT_PACKS_DIR", packs_dir)
        .output()
        .unwrap();
    assert!(res_after.status.success());
    let stdout_after = String::from_utf8_lossy(&res_after.stdout);
    assert!(stdout_after.contains("PDF    installed (QPDF Engine v11.9.1)"));
    assert!(stdout_after.contains("yes  pdf.merge"));

    let json_res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["capabilities", "--json"])
        .env("FOLDSHIFT_PACKS_DIR", packs_dir)
        .output()
        .unwrap();
    assert!(json_res.status.success());
    let json_ops: Vec<serde_json::Value> = serde_json::from_slice(&json_res.stdout).unwrap();
    let pdf_op = json_ops.iter().find(|op| op["id"] == "pdf.merge").unwrap();
    assert_eq!(pdf_op["available"], true);

    // Verify packs list reflects installed health
    let list_res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["packs", "list"])
        .env("FOLDSHIFT_PACKS_DIR", packs_dir)
        .output()
        .unwrap();
    assert!(list_res.status.success());
    let list_stdout = String::from_utf8_lossy(&list_res.stdout);
    assert!(list_stdout.contains("installed (v11.9.1) [healthy]"));

    // Corrupt the installation by removing the entrypoint binary
    std::fs::remove_file(&exe_path).unwrap();

    let res_corrupted = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .arg("capabilities")
        .env("FOLDSHIFT_PACKS_DIR", packs_dir)
        .output()
        .unwrap();
    let stdout_corrupted = String::from_utf8_lossy(&res_corrupted.stdout);
    assert!(stdout_corrupted.contains("PDF    not installed"));
    assert!(stdout_corrupted.contains("no   pdf.merge"));

    let list_corrupted = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["packs", "list"])
        .env("FOLDSHIFT_PACKS_DIR", packs_dir)
        .output()
        .unwrap();
    let list_corr_out = String::from_utf8_lossy(&list_corrupted.stdout);
    assert!(list_corr_out.contains("corrupted / entrypoint missing [failed]"));
}

fn create_sample_pdf(pages: u32) -> Vec<u8> {
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

fn mock_qpdf_bin() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_BIN_EXE_foldshift"))
        .parent()
        .unwrap()
        .join(if cfg!(windows) {
            "mock-qpdf.exe"
        } else {
            "mock-qpdf"
        })
}

#[test]
fn cli_pdf_subcommands_reject_when_uninstalled() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("doc.pdf");
    std::fs::write(&pdf, create_sample_pdf(1)).unwrap();
    let out = dir.path().join("out.pdf");

    // Clear any test overrides and packs dir
    let res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["pdf", "inspect"])
        .arg(&pdf)
        .env("FOLDSHIFT_PACKS_DIR", dir.path())
        .env_remove("FOLDSHIFT_QPDF_PATH")
        .output()
        .unwrap();
    assert!(!res.status.success());
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(stderr.contains("pdf capability is not installed"));

    let res_merge = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["pdf", "merge"])
        .arg(&pdf)
        .args(["-o"])
        .arg(&out)
        .env("FOLDSHIFT_PACKS_DIR", dir.path())
        .env_remove("FOLDSHIFT_QPDF_PATH")
        .output()
        .unwrap();
    assert!(!res_merge.status.success());
    let stderr_m = String::from_utf8_lossy(&res_merge.stderr);
    assert!(stderr_m.contains("pdf capability is not installed"));
}

#[test]
fn cli_pdf_operations_succeed_with_configured_engine() {
    let mock_path = mock_qpdf_bin();
    let dir = tempfile::tempdir().unwrap();
    let doc1 = dir.path().join("doc1.pdf");
    let doc2 = dir.path().join("doc2.pdf");
    let merged = dir.path().join("merged.pdf");
    let extracted = dir.path().join("extracted.pdf");

    std::fs::write(&doc1, create_sample_pdf(2)).unwrap();
    std::fs::write(&doc2, create_sample_pdf(3)).unwrap();

    // 1. Inspect
    let insp_res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["pdf", "inspect"])
        .arg(&doc1)
        .env("FOLDSHIFT_QPDF_PATH", &mock_path)
        .output()
        .unwrap();
    assert!(insp_res.status.success());
    let insp_out = String::from_utf8_lossy(&insp_res.stdout);
    assert!(insp_out.contains("Page Count  2"));

    // 2. Inspect with --json
    let json_insp_res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["pdf", "inspect", "--json"])
        .arg(&doc1)
        .env("FOLDSHIFT_QPDF_PATH", &mock_path)
        .output()
        .unwrap();
    assert!(json_insp_res.status.success());
    let json_val: serde_json::Value = serde_json::from_slice(&json_insp_res.stdout).unwrap();
    assert_eq!(json_val["format"], "PDF");
    assert_eq!(json_val["properties"]["page_count"], 2);

    // 3. Merge
    let merge_res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["pdf", "merge"])
        .arg(&doc1)
        .arg(&doc2)
        .args(["-o"])
        .arg(&merged)
        .env("FOLDSHIFT_QPDF_PATH", &mock_path)
        .output()
        .unwrap();
    assert!(merge_res.status.success());
    assert!(merged.exists());

    let merged_insp = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["pdf", "inspect", "--json"])
        .arg(&merged)
        .env("FOLDSHIFT_QPDF_PATH", &mock_path)
        .output()
        .unwrap();
    let merged_val: serde_json::Value = serde_json::from_slice(&merged_insp.stdout).unwrap();
    assert_eq!(merged_val["properties"]["page_count"], 5);

    // 4. Overwrite protection without --force
    let merge_fail = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["pdf", "merge"])
        .arg(&doc1)
        .arg(&doc2)
        .args(["-o"])
        .arg(&merged)
        .env("FOLDSHIFT_QPDF_PATH", &mock_path)
        .output()
        .unwrap();
    assert!(!merge_fail.status.success());
    let err_out = String::from_utf8_lossy(&merge_fail.stderr);
    assert!(err_out.contains("output already exists"));

    // 5. Extract
    let extract_res = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["pdf", "extract"])
        .arg(&merged)
        .arg(&extracted)
        .args(["--pages", "1-2"])
        .env("FOLDSHIFT_QPDF_PATH", &mock_path)
        .output()
        .unwrap();
    assert!(extract_res.status.success());
    assert!(extracted.exists());

    let extract_insp = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["pdf", "inspect", "--json"])
        .arg(&extracted)
        .env("FOLDSHIFT_QPDF_PATH", &mock_path)
        .output()
        .unwrap();
    let ext_val: serde_json::Value = serde_json::from_slice(&extract_insp.stdout).unwrap();
    assert_eq!(ext_val["properties"]["page_count"], 2);

    // 6. Invalid page range rejection
    let bad_range = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["pdf", "extract"])
        .arg(&merged)
        .arg(&extracted)
        .args(["--pages", "0", "--force"])
        .env("FOLDSHIFT_QPDF_PATH", &mock_path)
        .output()
        .unwrap();
    assert!(!bad_range.status.success());
    let bad_err = String::from_utf8_lossy(&bad_range.stderr);
    assert!(bad_err.contains("pages are 1-indexed"));
}
