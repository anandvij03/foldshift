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
