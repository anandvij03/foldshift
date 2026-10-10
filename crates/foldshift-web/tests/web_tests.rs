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
    stream.read_to_end(&mut response_bytes).await.unwrap();

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

#[tokio::test]
async fn test_index_and_capabilities_over_tcp() {
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
