use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();

    // Check version query
    if args.iter().any(|a| a == "-version") {
        println!("ffmpeg version 6.1.1-mock Copyright (c) 2000-2023 the FFmpeg developers");
        return;
    }

    // Check failure simulation
    if args.iter().any(|a| a == "--fail" || a.contains("corrupt")) {
        eprintln!("Error: corrupted or invalid media bitstream");
        std::process::exit(1);
    }

    // Check probe/inspect mode: -f null -
    if args.iter().any(|a| a == "null") {
        let is_video = args.iter().any(|a| {
            let lower = a.to_lowercase();
            lower.contains(".mp4")
                || lower.contains(".mkv")
                || lower.contains(".webm")
                || lower.contains(".mov")
                || lower.contains(".avi")
                || lower.contains(".flv")
                || lower.contains(".wmv")
                || lower.contains("video")
        });

        if is_video {
            eprintln!("Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'sample.mp4':");
            eprintln!("  Duration: 00:02:15.50, start: 0.000000, bitrate: 4500 kb/s");
            eprintln!(
                "  Stream #0:0: Video: h264 (High), yuv420p, 1920x1080 [SAR 1:1 DAR 16:9], 4200 kb/s, 30 fps, 30 tbr"
            );
            eprintln!("  Stream #0:1: Audio: aac (LC), 48000 Hz, stereo, fltp, 192 kb/s");
        } else {
            eprintln!("Input #0, mp3, from 'test.mp3':");
            eprintln!("  Duration: 00:03:15.50, start: 0.000000, bitrate: 320 kb/s");
            eprintln!("  Stream #0:0: Audio: mp3, 44100 Hz, stereo, fltp, 320 kb/s");
        }
        return;
    }

    // Conversion mode: last argument is output destination
    if let Some(out_path) = args.last()
        && !out_path.starts_with('-')
    {
        let p = Path::new(out_path);
        if let Some(parent) = p.parent()
            && !parent.as_os_str().is_empty()
        {
            let _ = fs::create_dir_all(parent);
        }

        let is_audio_extract = args.iter().any(|a| a == "-vn");
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_video_output = matches!(
            ext.as_str(),
            "mp4" | "mkv" | "webm" | "mov" | "avi" | "wmv" | "flv" | "m4v"
        ) || args
            .iter()
            .any(|a| a == "-vf" || a == "-crf" || a == "-c:v" || a == "-b:v");

        let payload: &[u8] = if is_audio_extract {
            b"MOCK_EXTRACTED_AUDIO_PAYLOAD"
        } else if is_video_output {
            b"MOCK_VIDEO_PAYLOAD"
        } else {
            b"MOCK_AUDIO_PAYLOAD_1234567890"
        };

        if let Err(e) = fs::write(p, payload) {
            eprintln!("Failed to write mock output: {}", e);
            std::process::exit(1);
        }
        eprintln!("Output #0, converted successfully to {}", out_path);
        return;
    }

    eprintln!("Invalid mock-ffmpeg invocation arguments: {:?}", args);
    std::process::exit(1);
}
