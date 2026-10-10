use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum::{
    Router,
    extract::{DefaultBodyLimit, Multipart},
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post},
};
use foldshift_image::EncodeOptions;
use serde::Serialize;
use serde_json::json;

pub const INDEX_HTML: &str = include_str!("index.html");

#[derive(Serialize)]
pub struct SystemStatus {
    pub version: &'static str,
    pub supported_extensions: &'static [&'static str],
    pub capabilities: Vec<foldshift_core::OperationInfo>,
}

pub fn app() -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/api/capabilities", get(api_capabilities))
        .route("/api/packs", get(api_packs))
        .route("/api/inspect", post(api_inspect))
        .route("/api/convert", post(api_convert))
        .route("/api/resize", post(api_resize))
        .route("/api/pdf/merge", post(api_pdf_merge))
        .route("/api/pdf/extract", post(api_pdf_extract))
        .layer(DefaultBodyLimit::max(100 * 102_400)) // 100 MB limit
}

pub async fn run(addr: SocketAddr, auto_open: bool) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let url = format!("http://{}", addr);

    println!("====================================================");
    println!("  FoldShift Web UI v0.1.0");
    println!("  Local-first file processing. Files stay on device.");
    println!("  Running at: {}", url);
    println!("  Press Ctrl+C to terminate.");
    println!("====================================================");

    if auto_open {
        let _ = open_browser(&url);
    }

    axum::serve(listener, app()).await?;
    Ok(())
}

pub fn open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    Ok(())
}

async fn serve_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn api_capabilities() -> Json<SystemStatus> {
    let is_pdf_available = foldshift_core::pack::is_capability_installed("pdf");
    let is_media_available = foldshift_core::pack::is_capability_installed("media");
    Json(SystemStatus {
        version: "0.1.0",
        supported_extensions: foldshift_image::SUPPORTED_EXTENSIONS,
        capabilities: foldshift_core::operations_with(|cap| match cap {
            "image" => true,
            "pdf" => is_pdf_available,
            "media" => is_media_available,
            _ => false,
        }),
    })
}

async fn api_packs() -> Json<Vec<foldshift_core::pack::PackStatus>> {
    Json(foldshift_core::pack::list_pack_statuses())
}

async fn api_inspect(
    mut multipart: Multipart,
) -> Result<Json<foldshift_core::Inspection>, (StatusCode, Json<serde_json::Value>)> {
    let mut file_bytes: Option<(String, Vec<u8>)> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            let filename = field.file_name().unwrap_or("unnamed").to_string();
            let bytes = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Failed reading file bytes: {}", e) })),
                )
            })?;
            file_bytes = Some((filename, bytes.to_vec()));
            break;
        }
    }

    let (filename, bytes) = file_bytes.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No file field uploaded" })),
        )
    })?;

    let ext = Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("tmp");

    let temp_file = tempfile::Builder::new()
        .suffix(&format!(".{}", ext))
        .tempfile()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        })?;

    std::fs::write(temp_file.path(), &bytes).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    let inspection = if ext.eq_ignore_ascii_case("pdf") {
        foldshift_pdf::inspect(temp_file.path()).map_err(|e| {
            let status = match e {
                foldshift_core::FcError::CapabilityUnavailable(_) => {
                    StatusCode::SERVICE_UNAVAILABLE
                }
                _ => StatusCode::UNPROCESSABLE_ENTITY,
            };
            (status, Json(json!({ "error": e.to_string() })))
        })?
    } else {
        foldshift_image::inspect(temp_file.path()).map_err(|e| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": e.to_string() })),
            )
        })?
    };

    let mut modified = inspection;
    modified.path = PathBuf::from(filename);

    Ok(Json(modified))
}

async fn api_convert(
    mut multipart: Multipart,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let mut file_data: Option<(String, Vec<u8>)> = None;
    let mut target_format = String::from("webp");
    let mut quality: u8 = 82;
    let force = true;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or("input").to_string();
                let bytes = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": format!("Upload error: {}", e) })),
                    )
                })?;
                file_data = Some((filename, bytes.to_vec()));
            }
            "format" => {
                if let Ok(text) = field.text().await {
                    let trimmed = text.trim().to_lowercase();
                    if !trimmed.is_empty() {
                        target_format = trimmed;
                    }
                }
            }
            "quality" => {
                if let Some(val) = field
                    .text()
                    .await
                    .ok()
                    .and_then(|t| t.trim().parse::<u8>().ok())
                {
                    quality = val.clamp(1, 100);
                }
            }
            _ => {}
        }
    }

    let (filename, bytes) = file_data.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No file uploaded" })),
        )
    })?;

    let in_ext = Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("tmp");

    let temp_input = tempfile::Builder::new()
        .suffix(&format!(".{}", in_ext))
        .tempfile()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        })?;

    std::fs::write(temp_input.path(), &bytes).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    let temp_dir = tempfile::tempdir().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;
    let output_path = temp_dir.path().join(format!("output.{}", target_format));

    let options = EncodeOptions { quality, force };
    foldshift_image::convert(temp_input.path(), &output_path, options).map_err(|e| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    let out_bytes = std::fs::read(&output_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    let stem = Path::new(&filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let download_name = format!("{}.{}", stem, target_format);

    let mime_type = match target_format.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        _ => "application/octet-stream",
    };

    let mut response = out_bytes.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime_type).unwrap(),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", download_name)).unwrap(),
    );

    Ok(response)
}

async fn api_resize(
    mut multipart: Multipart,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let mut file_data: Option<(String, Vec<u8>)> = None;
    let mut target_format = String::new();
    let mut quality: u8 = 82;
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;
    let force = true;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or("input").to_string();
                let bytes = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": format!("Upload error: {}", e) })),
                    )
                })?;
                file_data = Some((filename, bytes.to_vec()));
            }
            "format" => {
                if let Ok(text) = field.text().await {
                    let trimmed = text.trim().to_lowercase();
                    if !trimmed.is_empty() {
                        target_format = trimmed;
                    }
                }
            }
            "width" => {
                if let Some(val) = field
                    .text()
                    .await
                    .ok()
                    .and_then(|t| t.trim().parse::<u32>().ok())
                    .filter(|&v| v > 0)
                {
                    width = Some(val);
                }
            }
            "height" => {
                if let Some(val) = field
                    .text()
                    .await
                    .ok()
                    .and_then(|t| t.trim().parse::<u32>().ok())
                    .filter(|&v| v > 0)
                {
                    height = Some(val);
                }
            }
            "quality" => {
                if let Some(val) = field
                    .text()
                    .await
                    .ok()
                    .and_then(|t| t.trim().parse::<u8>().ok())
                {
                    quality = val.clamp(1, 100);
                }
            }
            _ => {}
        }
    }

    let (filename, bytes) = file_data.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No file uploaded" })),
        )
    })?;

    if width.is_none() && height.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Resize requires at least width or height" })),
        ));
    }

    let in_ext = Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();

    if target_format.is_empty() {
        target_format = in_ext.clone();
    }

    let temp_input = tempfile::Builder::new()
        .suffix(&format!(".{}", in_ext))
        .tempfile()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        })?;

    std::fs::write(temp_input.path(), &bytes).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    let temp_dir = tempfile::tempdir().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;
    let output_path = temp_dir.path().join(format!("output.{}", target_format));

    let options = EncodeOptions { quality, force };
    let (out_w, out_h) =
        foldshift_image::resize(temp_input.path(), &output_path, width, height, options).map_err(
            |e| {
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({ "error": e.to_string() })),
                )
            },
        )?;

    let out_bytes = std::fs::read(&output_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    let stem = Path::new(&filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let download_name = format!("{}_{}x{}.{}", stem, out_w, out_h, target_format);

    let mime_type = match target_format.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        _ => "application/octet-stream",
    };

    let mut response = out_bytes.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime_type).unwrap(),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", download_name)).unwrap(),
    );

    Ok(response)
}

async fn api_pdf_merge(
    mut multipart: Multipart,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let mut files = Vec::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        if name == "files" || name == "file" {
            let filename = field.file_name().unwrap_or("unnamed.pdf").to_string();
            let bytes = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Failed reading file bytes: {}", e) })),
                )
            })?;
            files.push((filename, bytes.to_vec()));
        }
    }

    if files.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No PDF files uploaded" })),
        ));
    }

    let temp_dir = tempfile::tempdir().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    let mut input_paths = Vec::new();
    for (i, (_name, bytes)) in files.iter().enumerate() {
        let in_path = temp_dir.path().join(format!("input_{}.pdf", i));
        std::fs::write(&in_path, bytes).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        })?;
        input_paths.push(in_path);
    }

    let output_path = temp_dir.path().join("merged.pdf");
    foldshift_pdf::merge(
        &input_paths,
        &output_path,
        foldshift_pdf::MergeOptions { force: true },
    )
    .map_err(|e| {
        let status = match e {
            foldshift_core::FcError::CapabilityUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        (status, Json(json!({ "error": e.to_string() })))
    })?;

    let out_bytes = std::fs::read(&output_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    let mut response = out_bytes.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"merged.pdf\""),
    );

    Ok(response)
}

async fn api_pdf_extract(
    mut multipart: Multipart,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let mut file_bytes: Option<(String, Vec<u8>)> = None;
    let mut pages_spec: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            let filename = field.file_name().unwrap_or("unnamed.pdf").to_string();
            let bytes = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Failed reading file bytes: {}", e) })),
                )
            })?;
            file_bytes = Some((filename, bytes.to_vec()));
        } else if name == "pages" {
            pages_spec = field.text().await.ok();
        }
    }

    let (filename, bytes) = file_bytes.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No file field uploaded" })),
        )
    })?;

    let pages = pages_spec.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Missing 'pages' parameter" })),
        )
    })?;

    let temp_dir = tempfile::tempdir().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    let in_path = temp_dir.path().join("input.pdf");
    std::fs::write(&in_path, &bytes).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    let output_path = temp_dir.path().join("extracted.pdf");
    foldshift_pdf::extract(
        &in_path,
        &output_path,
        foldshift_pdf::ExtractOptions {
            force: true,
            pages: pages.clone(),
        },
    )
    .map_err(|e| {
        let status = match e {
            foldshift_core::FcError::CapabilityUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        (status, Json(json!({ "error": e.to_string() })))
    })?;

    let out_bytes = std::fs::read(&output_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    let stem = Path::new(&filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    let safe_pages = pages.replace(',', "_");
    let download_name = format!("{}_pages_{}.pdf", stem, safe_pages);

    let mut response = out_bytes.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", download_name)).unwrap(),
    );

    Ok(response)
}
