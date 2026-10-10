use std::process::Command;

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

#[test]
fn invalid_options_return_failure() {
    let result = Command::new(env!("CARGO_BIN_EXE_foldshift"))
        .args(["convert", "a.png", "b.jpg", "--quality", "101"])
        .output()
        .unwrap();
    assert!(!result.status.success());
}

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
