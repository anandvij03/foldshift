use image::{DynamicImage, GenericImageView, ImageFormat, Rgb, RgbImage};
use std::io::Cursor;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

fn create_sample_png(w: u32, h: u32) -> Vec<u8> {
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, Rgb([200, 50, 80])));
    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .unwrap();
    buf
}

fn build_multipart(
    boundary: &str,
    file_part: Option<(&str, &str, &[u8])>, // field_name, filename, data
    text_fields: &[(&str, &str)],           // name, val
) -> Vec<u8> {
    let mut body = Vec::new();
    for (name, val) in text_fields {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{}\"\r\n\r\n{}\r\n",
                name, val
            )
            .as_bytes(),
        );
    }
    if let Some((field, filename, data)) = file_part {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!(
                "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\nContent-Type: application/octet-stream\r\n\r\n",
                field, filename
            )
            .as_bytes(),
        );
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    body
}

async fn send_request(
    addr: std::net::SocketAddr,
    method: &str,
    path: &str,
    content_type: &str,
    body: &[u8],
) -> (String, Vec<u8>) {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let header = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        method,
        path,
        addr,
        content_type,
        body.len()
    );
    stream.write_all(header.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();

    let mut response_bytes = Vec::new();
    match stream.read_to_end(&mut response_bytes).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {}
        Err(e) => panic!("read_to_end failed: {}", e),
    }

    let header_end = response_bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap_or(response_bytes.len());

    let headers_part = String::from_utf8_lossy(&response_bytes[..header_end]).to_string();
    let body_part = if header_end + 4 <= response_bytes.len() {
        response_bytes[header_end + 4..].to_vec()
    } else {
        Vec::new()
    };

    (headers_part, body_part)
}

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn test_index_and_capabilities_over_tcp() {
    let _guard = ENV_LOCK.lock().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let app = foldshift_web::app();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // 1. GET /
    let (head, _) = send_request(addr, "GET", "/", "text/plain", b"").await;
    assert!(head.starts_with("HTTP/1.1 200 OK"));
    assert!(head.contains("text/html"));

    // 2. GET /api/capabilities
    let (head2, body2) =
        send_request(addr, "GET", "/api/capabilities", "application/json", b"").await;
    assert!(head2.starts_with("HTTP/1.1 200 OK"));
    let body_str = String::from_utf8_lossy(&body2);
    assert!(body_str.contains("\"version\":\"0.1.0\""));
    assert!(body_str.contains("\"image.convert\""));
}

#[tokio::test]
async fn test_api_inspect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, foldshift_web::app()).await.unwrap();
    });

    let png_bytes = create_sample_png(120, 80);
    let boundary = "BOUNDARY123";
    let body = build_multipart(boundary, Some(("file", "sample.png", &png_bytes)), &[]);
    let ct = format!("multipart/form-data; boundary={}", boundary);

    let (head, resp_body) = send_request(addr, "POST", "/api/inspect", &ct, &body).await;
    assert!(
        head.starts_with("HTTP/1.1 200 OK"),
        "Expected 200 OK, got: {}",
        head
    );

    let json: serde_json::Value = serde_json::from_slice(&resp_body).unwrap();
    assert_eq!(json["format"], "PNG");
    assert_eq!(json["properties"]["width"], 120);
    assert_eq!(json["properties"]["height"], 80);
}

#[tokio::test]
async fn test_api_convert() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, foldshift_web::app()).await.unwrap();
    });

    let png_bytes = create_sample_png(60, 40);
    let boundary = "BOUNDARY456";
    let body = build_multipart(
        boundary,
        Some(("file", "photo.png", &png_bytes)),
        &[("format", "webp"), ("quality", "90")],
    );
    let ct = format!("multipart/form-data; boundary={}", boundary);

    let (head, resp_body) = send_request(addr, "POST", "/api/convert", &ct, &body).await;
    assert!(
        head.starts_with("HTTP/1.1 200 OK"),
        "Expected 200 OK, got: {}",
        head
    );
    assert!(head.contains("image/webp"));
    assert!(head.contains("photo.webp"));

    // Verify converted image bytes can be decoded
    let decoded = image::load_from_memory(&resp_body).unwrap();
    assert_eq!(decoded.dimensions(), (60, 40));
}

#[tokio::test]
async fn test_api_resize() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, foldshift_web::app()).await.unwrap();
    });

    let png_bytes = create_sample_png(100, 50);
    let boundary = "BOUNDARY789";
    let body = build_multipart(
        boundary,
        Some(("file", "graphic.png", &png_bytes)),
        &[("width", "50"), ("height", "50"), ("format", "png")],
    );
    let ct = format!("multipart/form-data; boundary={}", boundary);

    let (head, resp_body) = send_request(addr, "POST", "/api/resize", &ct, &body).await;
    assert!(
        head.starts_with("HTTP/1.1 200 OK"),
        "Expected 200 OK, got: {}",
        head
    );
    assert!(head.contains("graphic_50x25.png"));

    let decoded = image::load_from_memory(&resp_body).unwrap();
    assert_eq!(decoded.dimensions(), (50, 25));
}

#[tokio::test]
async fn test_api_error_handling() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, foldshift_web::app()).await.unwrap();
    });

    // 1. Missing file field
    let boundary = "BOUNDARYERR";
    let body = build_multipart(boundary, None, &[("format", "webp")]);
    let ct = format!("multipart/form-data; boundary={}", boundary);

    let (head, resp_body) = send_request(addr, "POST", "/api/convert", &ct, &body).await;
    assert!(head.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(String::from_utf8_lossy(&resp_body).contains("error"));

    // 2. Corrupted file bytes
    let bad_body = build_multipart(boundary, Some(("file", "bad.png", b"not an image")), &[]);
    let (head2, resp_body2) = send_request(addr, "POST", "/api/inspect", &ct, &bad_body).await;
    assert!(head2.starts_with("HTTP/1.1 422 Unprocessable Entity"));
    assert!(String::from_utf8_lossy(&resp_body2).contains("error"));
}

fn mock_qpdf_bin() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join(if cfg!(windows) {
        "mock-qpdf.exe"
    } else {
        "mock-qpdf"
    })
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

#[tokio::test]
async fn test_api_pdf_endpoints() {
    let _guard = ENV_LOCK.lock().await;
    let mock_path = mock_qpdf_bin();
    unsafe { std::env::set_var("FOLDSHIFT_QPDF_PATH", &mock_path) };

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, foldshift_web::app()).await.unwrap();
    });

    let pdf1 = create_sample_pdf(2);
    let boundary = "BOUNDARYPDF";

    // 1. Inspect PDF via /api/inspect
    let inspect_body = build_multipart(boundary, Some(("file", "sample.pdf", &pdf1)), &[]);
    let ct = format!("multipart/form-data; boundary={}", boundary);
    let (head_insp, body_insp) =
        send_request(addr, "POST", "/api/inspect", &ct, &inspect_body).await;
    assert!(head_insp.starts_with("HTTP/1.1 200 OK"));
    let insp_data: serde_json::Value = serde_json::from_slice(&body_insp).unwrap();
    assert_eq!(insp_data["format"], "PDF");
    assert_eq!(insp_data["properties"]["page_count"], 2);

    // 2. Extract PDF via /api/pdf/extract
    let extract_body = build_multipart(
        boundary,
        Some(("file", "sample.pdf", &pdf1)),
        &[("pages", "1")],
    );
    let (head_ext, body_ext) =
        send_request(addr, "POST", "/api/pdf/extract", &ct, &extract_body).await;
    assert!(head_ext.starts_with("HTTP/1.1 200 OK"));
    assert!(head_ext.contains("application/pdf"));
    assert!(!body_ext.is_empty());

    unsafe { std::env::remove_var("FOLDSHIFT_QPDF_PATH") };
}

#[tokio::test]
async fn test_api_packs_install_validation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, foldshift_web::app()).await.unwrap();
    });

    let boundary = "BOUNDARYPACK";

    // 1. Missing file field entirely -> 400 Bad Request
    let empty_body = build_multipart(boundary, None, &[("capability", "pdf")]);
    let ct = format!("multipart/form-data; boundary={}", boundary);
    let (head_empty, body_empty) =
        send_request(addr, "POST", "/api/packs/install", &ct, &empty_body).await;
    assert!(
        head_empty.starts_with("HTTP/1.1 400 Bad Request"),
        "Expected 400, got: {}",
        head_empty
    );
    assert!(String::from_utf8_lossy(&body_empty).contains("error"));

    // 2. Corrupted / non-zip archive payload -> 422 Unprocessable Entity
    let bad_zip_body = build_multipart(
        boundary,
        Some(("file", "invalid_pack.zip", b"not a valid zip file payload")),
        &[("capability", "pdf")],
    );
    let (head_bad, body_bad) =
        send_request(addr, "POST", "/api/packs/install", &ct, &bad_zip_body).await;
    assert!(
        head_bad.starts_with("HTTP/1.1 422 Unprocessable Entity"),
        "Expected 422, got: {}",
        head_bad
    );
    let err_json: serde_json::Value = serde_json::from_slice(&body_bad).unwrap();
    assert!(err_json["error"].is_string());
}

#[tokio::test]
async fn test_api_capabilities_dynamic_pdf_detection() {
    let _guard = ENV_LOCK.lock().await;
    let mock_path = mock_qpdf_bin();
    unsafe { std::env::set_var("FOLDSHIFT_QPDF_PATH", &mock_path) };

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, foldshift_web::app()).await.unwrap();
    });

    let (head, body) =
        send_request(addr, "GET", "/api/capabilities", "application/json", b"").await;
    assert!(head.starts_with("HTTP/1.1 200 OK"));
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["pdf_available"], true);
    assert!(
        json["pdf_source"]
            .as_str()
            .unwrap()
            .contains("FOLDSHIFT_QPDF_PATH")
    );

    unsafe { std::env::remove_var("FOLDSHIFT_QPDF_PATH") };
}

fn mock_ffmpeg_bin() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join(if cfg!(windows) {
        "mock-ffmpeg.exe"
    } else {
        "mock-ffmpeg"
    })
}

#[tokio::test]
async fn test_api_capabilities_dynamic_media_detection() {
    let _guard = ENV_LOCK.lock().await;
    let mock_path = mock_ffmpeg_bin();
    unsafe { std::env::set_var("FOLDSHIFT_FFMPEG_PATH", &mock_path) };

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, foldshift_web::app()).await.unwrap();
    });

    let (head, body) =
        send_request(addr, "GET", "/api/capabilities", "application/json", b"").await;
    assert!(head.starts_with("HTTP/1.1 200 OK"));
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["media_available"], true);
    assert!(
        json["media_source"]
            .as_str()
            .unwrap()
            .contains("FOLDSHIFT_FFMPEG_PATH")
    );

    unsafe { std::env::remove_var("FOLDSHIFT_FFMPEG_PATH") };
}

#[tokio::test]
async fn test_api_audio_endpoints() {
    let _guard = ENV_LOCK.lock().await;
    let mock_path = mock_ffmpeg_bin();
    unsafe { std::env::set_var("FOLDSHIFT_FFMPEG_PATH", &mock_path) };

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, foldshift_web::app()).await.unwrap();
    });

    let boundary = "BOUNDARYAUDIO";
    let ct = format!("multipart/form-data; boundary={}", boundary);

    // 1. POST /api/audio/inspect
    let audio_data = b"MOCK_AUDIO_DATA_FOR_TEST";
    let inspect_body = build_multipart(boundary, Some(("file", "test_track.mp3", audio_data)), &[]);
    let (head_insp, body_insp) =
        send_request(addr, "POST", "/api/audio/inspect", &ct, &inspect_body).await;
    assert!(
        head_insp.starts_with("HTTP/1.1 200 OK"),
        "Expected 200 OK, got: {}",
        head_insp
    );
    let insp_json: serde_json::Value = serde_json::from_slice(&body_insp).unwrap();
    assert_eq!(insp_json["format"], "MP3");
    assert_eq!(insp_json["properties"]["duration"], "00:03:15.50");

    // 2. POST /api/audio/convert
    let convert_body = build_multipart(
        boundary,
        Some(("file", "track.wav", audio_data)),
        &[("format", "mp3"), ("bitrate", "320k")],
    );
    let (head_conv, body_conv) =
        send_request(addr, "POST", "/api/audio/convert", &ct, &convert_body).await;
    assert!(
        head_conv.starts_with("HTTP/1.1 200 OK"),
        "Expected 200 OK, got: {}",
        head_conv
    );
    assert!(head_conv.contains("audio/mpeg"));
    assert!(head_conv.contains("track.mp3"));
    assert!(!body_conv.is_empty());

    unsafe { std::env::remove_var("FOLDSHIFT_FFMPEG_PATH") };
}

#[tokio::test]
async fn test_api_video_endpoints() {
    let _guard = ENV_LOCK.lock().await;
    let mock_path = mock_ffmpeg_bin();
    unsafe { std::env::set_var("FOLDSHIFT_FFMPEG_PATH", &mock_path) };

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, foldshift_web::app()).await.unwrap();
    });

    let boundary = "BOUNDARYVIDEO";
    let ct = format!("multipart/form-data; boundary={}", boundary);

    // 1. POST /api/video/inspect
    let video_data = b"MOCK_VIDEO_DATA_FOR_TEST";
    let inspect_body = build_multipart(boundary, Some(("file", "test_clip.mp4", video_data)), &[]);
    let (head_insp, body_insp) =
        send_request(addr, "POST", "/api/video/inspect", &ct, &inspect_body).await;
    assert!(
        head_insp.starts_with("HTTP/1.1 200 OK"),
        "Expected 200 OK, got: {}",
        head_insp
    );
    let insp_json: serde_json::Value = serde_json::from_slice(&body_insp).unwrap();
    assert_eq!(insp_json["format"], "MP4");
    assert_eq!(insp_json["media_kind"], "video");
    assert_eq!(insp_json["properties"]["duration"], "00:02:15.50");
    assert_eq!(insp_json["properties"]["video_codec"], "h264");
    assert_eq!(insp_json["properties"]["resolution"], "1920x1080");

    // 2. POST /api/video/convert
    let convert_body = build_multipart(
        boundary,
        Some(("file", "clip.mp4", video_data)),
        &[
            ("format", "webm"),
            ("width", "1280"),
            ("fps", "30"),
            ("crf", "23"),
        ],
    );
    let (head_conv, body_conv) =
        send_request(addr, "POST", "/api/video/convert", &ct, &convert_body).await;
    assert!(
        head_conv.starts_with("HTTP/1.1 200 OK"),
        "Expected 200 OK, got: {}",
        head_conv
    );
    assert!(head_conv.contains("video/webm"));
    assert!(head_conv.contains("clip.webm"));
    assert!(!body_conv.is_empty());

    // 3. POST /api/video/extract-audio
    let extract_body = build_multipart(
        boundary,
        Some(("file", "clip.mp4", video_data)),
        &[("format", "mp3")],
    );
    let (head_ext, body_ext) =
        send_request(addr, "POST", "/api/video/extract-audio", &ct, &extract_body).await;
    assert!(
        head_ext.starts_with("HTTP/1.1 200 OK"),
        "Expected 200 OK, got: {}",
        head_ext
    );
    assert!(head_ext.contains("audio/mpeg"));
    assert!(head_ext.contains("clip.mp3"));
    assert!(!body_ext.is_empty());

    unsafe { std::env::remove_var("FOLDSHIFT_FFMPEG_PATH") };
}

#[tokio::test]
async fn test_api_manifest() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, foldshift_web::app()).await.unwrap();
    });

    let (head, body) = send_request(addr, "GET", "/api/manifest", "application/json", &[]).await;
    assert!(head.starts_with("HTTP/1.1 200 OK"), "Got head: {}", head);

    let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(val["manifest"]["schema_version"], "1.0.0");
    assert!(val["manifest"]["artifacts"].is_array());
    assert!(val["engine_packs"].is_array());
    assert!(val["capabilities"].is_array());
}

#[tokio::test]
async fn test_api_system_shutdown() {
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    let app = foldshift_web::app_with_state(foldshift_web::WebState { shutdown_tx });

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                while !*shutdown_rx.borrow() {
                    if shutdown_rx.changed().await.is_err() {
                        break;
                    }
                }
            })
            .await
            .unwrap();
    });

    let (head, body) =
        send_request(addr, "POST", "/api/system/shutdown", "application/json", &[]).await;
    assert!(head.starts_with("HTTP/1.1 200 OK"), "Got head: {}", head);

    let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(val["status"], "shutting_down");

    // The server handle should finish cleanly within a short timeout
    let finished = tokio::time::timeout(std::time::Duration::from_secs(5), server_handle).await;
    assert!(finished.is_ok(), "Server failed to shut down within timeout");
}
