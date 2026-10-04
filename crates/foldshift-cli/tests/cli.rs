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
