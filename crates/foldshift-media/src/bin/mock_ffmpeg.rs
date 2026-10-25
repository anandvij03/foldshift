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
        eprintln!("Error: corrupted or invalid audio bitstream");
        std::process::exit(1);
    }

    // Check probe/inspect mode: -f null -
    if args.iter().any(|a| a == "null") {
        eprintln!("Input #0, mp3, from 'test.mp3':");
        eprintln!("  Duration: 00:03:15.50, start: 0.000000, bitrate: 320 kb/s");
        eprintln!("  Stream #0:0: Audio: mp3, 44100 Hz, stereo, fltp, 320 kb/s");
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

        // Write mock converted audio payload
        let mock_audio = b"MOCK_AUDIO_PAYLOAD_1234567890";
        if let Err(e) = fs::write(p, mock_audio) {
            eprintln!("Failed to write mock audio output: {}", e);
            std::process::exit(1);
        }
        eprintln!("Output #0, converted successfully to {}", out_path);
        return;
    }

    eprintln!("Invalid mock-ffmpeg invocation arguments: {:?}", args);
    std::process::exit(1);
}
