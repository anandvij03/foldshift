use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use foldshift_core::FcError;
use foldshift_media::{AudioConvertOptions, batch_convert, convert, inspect};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn mock_ffmpeg_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mock-ffmpeg"))
}

#[test]
fn test_audio_inspect_with_mock_engine() {
    let _guard = ENV_LOCK.lock().unwrap();
    let mock = mock_ffmpeg_bin();
    unsafe { std::env::set_var("FOLDSHIFT_FFMPEG_PATH", &mock) };

    let dir = tempfile::tempdir().unwrap();
    let audio_file = dir.path().join("sample.mp3");
    fs::write(&audio_file, b"MOCK_SOURCE_AUDIO_BYTES").unwrap();

    let res = inspect(&audio_file).unwrap();
    assert_eq!(res.format, "MP3");
    assert_eq!(res.media_kind, foldshift_core::MediaKind::Audio);
    assert_eq!(res.properties.get("duration").unwrap(), "00:03:15.50");
    assert_eq!(res.properties.get("bitrate").unwrap(), "320 kb/s");
    assert_eq!(res.properties.get("codec").unwrap(), "mp3");
    assert_eq!(res.properties.get("sample_rate").unwrap(), "44100 Hz");
    assert_eq!(res.properties.get("channels").unwrap(), "stereo");

    unsafe { std::env::remove_var("FOLDSHIFT_FFMPEG_PATH") };
}

#[test]
fn test_audio_convert_and_same_file_policy() {
    let _guard = ENV_LOCK.lock().unwrap();
    let mock = mock_ffmpeg_bin();
    unsafe { std::env::set_var("FOLDSHIFT_FFMPEG_PATH", &mock) };

    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("input.wav");
    let output = dir.path().join("output.mp3");
    fs::write(&input, b"INITIAL_WAV_BYTES").unwrap();

    // 1. Successful conversion
    let options = AudioConvertOptions {
        bitrate: Some("192k".into()),
        sample_rate: Some(44100),
        channels: Some(2),
        force: false,
    };
    convert(&input, &output, options.clone()).unwrap();
    assert!(output.exists());
    assert_eq!(fs::read(&output).unwrap(), b"MOCK_AUDIO_PAYLOAD_1234567890");

    // 2. Output exists without force -> fails
    let err = convert(&input, &output, options).unwrap_err();
    assert!(matches!(err, FcError::OutputExists(_)));

    // 3. Same-file without force -> fails
    let same_err = convert(
        &input,
        &input,
        AudioConvertOptions {
            force: false,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(matches!(same_err, FcError::OutputExists(_)));

    // 4. Same-file with force -> succeeds in place
    convert(
        &input,
        &input,
        AudioConvertOptions {
            force: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(fs::read(&input).unwrap(), b"MOCK_AUDIO_PAYLOAD_1234567890");

    unsafe { std::env::remove_var("FOLDSHIFT_FFMPEG_PATH") };
}

#[test]
fn test_failed_conversion_preserves_destination() {
    let _guard = ENV_LOCK.lock().unwrap();
    let mock = mock_ffmpeg_bin();
    unsafe { std::env::set_var("FOLDSHIFT_FFMPEG_PATH", &mock) };

    let dir = tempfile::tempdir().unwrap();
    let corrupt_input = dir.path().join("corrupt_input.wav");
    let existing_output = dir.path().join("existing.mp3");

    fs::write(&corrupt_input, b"corrupted bytes").unwrap();
    fs::write(&existing_output, b"PRESERVED_ORIGINAL_CONTENT").unwrap();

    let err = convert(
        &corrupt_input,
        &existing_output,
        AudioConvertOptions {
            force: true,
            ..Default::default()
        },
    )
    .unwrap_err();

    assert!(matches!(err, FcError::Conversion(_)));
    // Destination file was untouched and preserved
    assert_eq!(
        fs::read(&existing_output).unwrap(),
        b"PRESERVED_ORIGINAL_CONTENT"
    );

    unsafe { std::env::remove_var("FOLDSHIFT_FFMPEG_PATH") };
}

#[test]
fn test_batch_audio_convert_with_independent_failures() {
    let _guard = ENV_LOCK.lock().unwrap();
    let mock = mock_ffmpeg_bin();
    unsafe { std::env::set_var("FOLDSHIFT_FFMPEG_PATH", &mock) };

    let dir = tempfile::tempdir().unwrap();
    let good1 = dir.path().join("track1.wav");
    let corrupt = dir.path().join("corrupt_track.wav");
    let good2 = dir.path().join("track2.wav");

    fs::write(&good1, b"TRACK_1_BYTES").unwrap();
    fs::write(&corrupt, b"CORRUPT_BYTES").unwrap();
    fs::write(&good2, b"TRACK_2_BYTES").unwrap();

    let out_dir = dir.path().join("converted_audio");
    let cancel_token = foldshift_core::batch::CancellationToken::new();

    let summary = batch_convert(
        &[good1, corrupt, good2],
        "mp3",
        &out_dir,
        Some("{index:02}_{stem}.{ext}"),
        AudioConvertOptions::default(),
        &cancel_token,
        |_| {},
    )
    .unwrap();

    assert_eq!(summary.total, 3);
    assert_eq!(summary.succeeded, 2);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.cancelled, 0);

    assert!(out_dir.join("01_track1.mp3").exists());
    assert!(!out_dir.join("02_corrupt_track.mp3").exists());
    assert!(out_dir.join("03_track2.mp3").exists());

    unsafe { std::env::remove_var("FOLDSHIFT_FFMPEG_PATH") };
}
