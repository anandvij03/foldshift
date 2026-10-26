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
use tokio::sync::watch;
use serde::Serialize;
use serde_json::json;

pub const INDEX_HTML: &str = include_str!("index.html");
pub const MANIFEST_JSON: &str = include_str!("../../../dist/manifest.json");

#[derive(Serialize)]
pub struct SystemStatus {
    pub version: &'static str,
    pub supported_extensions: &'static [&'static str],
    pub supported_audio_extensions: &'static [&'static str],
    pub supported_video_extensions: &'static [&'static str],
    pub capabilities: Vec<foldshift_core::OperationInfo>,
    pub pdf_available: bool,
    pub pdf_source: Option<String>,
    pub media_available: bool,
    pub media_source: Option<String>,
}

#[derive(Clone)]
pub struct WebState {
    pub shutdown_tx: watch::Sender<bool>,
}

pub fn app_with_state(state: WebState) -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/api/capabilities", get(api_capabilities))
        .route("/api/manifest", get(api_manifest))
        .route("/api/system/shutdown", post(api_system_shutdown))
        .route("/api/packs", get(api_packs))
        .route("/api/packs/install", post(api_packs_install))
        .route("/api/inspect", post(api_inspect))
        .route("/api/convert", post(api_convert))
        .route("/api/resize", post(api_resize))
        .route("/api/pdf/merge", post(api_pdf_merge))
        .route("/api/pdf/extract", post(api_pdf_extract))
        .route("/api/audio/inspect", post(api_audio_inspect))
        .route("/api/audio/convert", post(api_audio_convert))
        .route("/api/video/inspect", post(api_video_inspect))
        .route("/api/video/convert", post(api_video_convert))
        .route("/api/video/extract-audio", post(api_video_extract_audio))
        .with_state(state)
        .layer(DefaultBodyLimit::max(100 * 102_400)) // 100 MB limit
}

pub fn app() -> Router {
    let (tx, _) = tokio::sync::watch::channel(false);
    app_with_state(WebState { shutdown_tx: tx })
}

pub async fn run(addr: SocketAddr, auto_open: bool) -> anyhow::Result<()> {
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

    let ctrl_tx = shutdown_tx.clone();
    let _ = ctrlc::set_handler(move || {
        println!("\nShutdown signal (Ctrl+C) received. Stopping server...");
        let _ = ctrl_tx.send(true);
    });

    let listener = tokio::net::TcpListener::bind(addr).await?;
    let url = format!("http://{}", addr);

    println!("====================================================");
    println!("  FoldShift Web UI v0.1.0");
    println!("  Local-first file processing. Files stay on device.");
    println!("  Running at: {}", url);
    println!("  Press Ctrl+C or use UI 'Shutdown' to terminate.");
    println!("====================================================");

    if auto_open {
        let _ = open_browser(&url);
    }

    let app = app_with_state(WebState { shutdown_tx });

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            while !*shutdown_rx.borrow() {
                if shutdown_rx.changed().await.is_err() {
                    break;
                }
            }
            println!("FoldShift local server cleanly terminated. Exiting.");
        })
        .await?;

    Ok(())
}

async fn api_manifest() -> impl IntoResponse {
    let manifest_val: serde_json::Value = serde_json::from_str(MANIFEST_JSON)
        .unwrap_or_else(|_| json!({ "error": "Invalid manifest json" }));

    let pdf_pack_installed = foldshift_core::pack::is_capability_installed("pdf");
    let media_pack_installed = foldshift_core::pack::is_capability_installed("media");

    let engine_packs = json!([
        {
            "id": "pdf",
            "name": "PDF Engine Pack",
            "engine": "qpdf",
            "version": "11.9.0",
            "description": "Subprocess adapter for structural PDF merge, page extraction, and inspection",
            "download_bytes": 12690000,
            "installed_bytes": 29780000,
            "installed": pdf_pack_installed,
            "license": "Apache-2.0",
            "shared": false,
            "install_cmd": "foldshift packs install pdf --from-file <path>"
        },
        {
            "id": "media",
            "name": "Media Engine Pack",
            "engine": "ffmpeg",
            "version": "7.0.2",
            "description": "Unified subprocess adapter for audio transcoding/inspection and video operations",
            "download_bytes": 33030000,
            "installed_bytes": 75490000,
            "installed": media_pack_installed,
            "license": "LGPL-2.1 / GPL-2.0+",
            "shared": true,
            "shared_capabilities": ["audio", "video"],
            "install_cmd": "foldshift packs install media --from-file <path>"
        }
    ]);

    Json(json!({
        "manifest": manifest_val,
        "engine_packs": engine_packs,
        "capabilities": foldshift_core::operations(),
    }))
}

async fn api_system_shutdown(
    axum::extract::State(state): axum::extract::State<WebState>,
) -> impl IntoResponse {
    let _ = state.shutdown_tx.send(true);
    (
        StatusCode::OK,
        Json(json!({
            "status": "shutting_down",
            "message": "FoldShift local server is stopping cleanly. No background daemons will remain."
        })),
    )
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
    let (is_pdf_available, pdf_source) = match foldshift_pdf::resolve_qpdf_engine() {
        Ok(path) => {
            let src = if foldshift_core::pack::is_capability_installed("pdf") {
                "Pack"
            } else if std::env::var("FOLDSHIFT_QPDF_PATH").is_ok() {
                "Environment variable (FOLDSHIFT_QPDF_PATH)"
            } else {
                "System PATH"
            };
            (true, Some(format!("{} ({})", src, path.display())))
        }
        Err(_) => (false, None),
    };
    let (is_media_available, media_source) = match foldshift_media::resolve_media_engine() {
        Ok(path) => {
            let src = if foldshift_core::pack::is_capability_installed("media") {
                "Pack"
            } else if std::env::var("FOLDSHIFT_FFMPEG_PATH").is_ok() {
                "Environment variable (FOLDSHIFT_FFMPEG_PATH)"
            } else if std::env::var("FOLDSHIFT_MEDIA_PATH").is_ok() {
                "Environment variable (FOLDSHIFT_MEDIA_PATH)"
            } else {
                "System PATH"
            };
            (true, Some(format!("{} ({})", src, path.display())))
        }
        Err(_) => (false, None),
    };
    Json(SystemStatus {
        version: "0.1.0",
        supported_extensions: foldshift_image::SUPPORTED_EXTENSIONS,
        supported_audio_extensions: foldshift_media::SUPPORTED_AUDIO_EXTENSIONS,
        supported_video_extensions: foldshift_media::SUPPORTED_VIDEO_EXTENSIONS,
        capabilities: foldshift_core::operations_with(|cap| match cap {
            "image" => true,
            "pdf" => is_pdf_available,
            "media" => is_media_available,
            _ => false,
        }),
        pdf_available: is_pdf_available,
        pdf_source,
        media_available: is_media_available,
        media_source,
    })
}

async fn api_packs() -> Json<Vec<foldshift_core::pack::PackStatus>> {
    Json(foldshift_core::pack::list_pack_statuses())
}

async fn api_packs_install(
    mut multipart: Multipart,
) -> Result<Json<foldshift_core::pack::InstalledPack>, (StatusCode, Json<serde_json::Value>)> {
    let mut file_bytes: Option<(String, Vec<u8>)> = None;
    let mut expected_capability: Option<String> = None;
    let mut expected_sha256: Option<String> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "file" | "pack" => {
                let filename = field.file_name().unwrap_or("pack.zip").to_string();
                let bytes = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": format!("Failed reading pack file bytes: {}", e) })),
                    )
                })?;
                file_bytes = Some((filename, bytes.to_vec()));
            }
            "capability" => {
                if let Ok(text) = field.text().await {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        expected_capability = Some(trimmed.to_string());
                    }
                }
            }
            "sha256" => {
                if let Ok(text) = field.text().await {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        expected_sha256 = Some(trimmed.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    let (_filename, bytes) = file_bytes.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No pack archive file uploaded" })),
        )
    })?;

    let temp_file = tempfile::Builder::new()
        .suffix(".zip")
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

    let options = foldshift_core::pack::InstallOptions {
        expected_capability: expected_capability.as_deref(),
        expected_sha256: expected_sha256.as_deref(),
    };

    let installed = foldshift_core::pack::install_pack_from_archive(
        temp_file.path(),
        &foldshift_core::pack::packs_dir(),
        &options,
    )
    .map_err(|e| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({ "error": e.to_string() })),
        )
    })?;

    Ok(Json(installed))
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
    } else if foldshift_media::is_supported_audio_extension(ext)
        || foldshift_media::is_supported_video_extension(ext)
    {
        foldshift_media::inspect(temp_file.path()).map_err(|e| {
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

async fn api_audio_inspect(
    mut multipart: Multipart,
) -> Result<Json<foldshift_core::Inspection>, (StatusCode, Json<serde_json::Value>)> {
    let mut file_bytes: Option<(String, Vec<u8>)> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            let filename = field.file_name().unwrap_or("unnamed.mp3").to_string();
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
        .unwrap_or("mp3");

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

    let inspection = foldshift_media::inspect(temp_file.path()).map_err(|e| {
        let status = match e {
            foldshift_core::FcError::CapabilityUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        (status, Json(json!({ "error": e.to_string() })))
    })?;

    let mut modified = inspection;
    modified.path = PathBuf::from(filename);

    Ok(Json(modified))
}

async fn api_audio_convert(
    mut multipart: Multipart,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let mut file_data: Option<(String, Vec<u8>)> = None;
    let mut target_format = String::from("mp3");
    let mut bitrate: Option<String> = None;
    let mut sample_rate: Option<u32> = None;
    let mut channels: Option<u8> = None;
    let force = true;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or("input.mp3").to_string();
                let bytes = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": format!("Upload error: {}", e) })),
                    )
                })?;
                file_data = Some((filename, bytes.to_vec()));
            }
            "format" | "to" => {
                if let Ok(text) = field.text().await {
                    let trimmed = text.trim().to_lowercase();
                    if !trimmed.is_empty() {
                        target_format = trimmed;
                    }
                }
            }
            "bitrate" => {
                if let Ok(text) = field.text().await {
                    let trimmed = text.trim().to_lowercase();
                    if !trimmed.is_empty() {
                        bitrate = Some(trimmed);
                    }
                }
            }
            "sample_rate" => {
                if let Some(val) = field
                    .text()
                    .await
                    .ok()
                    .and_then(|t| t.trim().parse::<u32>().ok())
                    .filter(|&v| v > 0)
                {
                    sample_rate = Some(val);
                }
            }
            "channels" => {
                if let Some(val) = field
                    .text()
                    .await
                    .ok()
                    .and_then(|t| t.trim().parse::<u8>().ok())
                    .filter(|&v| v > 0)
                {
                    channels = Some(val);
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
        .unwrap_or("mp3");

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

    let options = foldshift_media::AudioConvertOptions {
        bitrate,
        sample_rate,
        channels,
        force,
    };

    foldshift_media::convert(temp_input.path(), &output_path, options).map_err(|e| {
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
        .unwrap_or("output");
    let download_name = format!("{}.{}", stem, target_format);

    let mime_type = match target_format.as_str() {
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "opus" => "audio/opus",
        "wma" => "audio/x-ms-wma",
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

async fn api_video_inspect(
    mut multipart: Multipart,
) -> Result<Json<foldshift_core::Inspection>, (StatusCode, Json<serde_json::Value>)> {
    let mut file_data: Option<(String, Vec<u8>)> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            let filename = field.file_name().unwrap_or("sample.mp4").to_string();
            let bytes = field.bytes().await.map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Upload error: {}", e) })),
                )
            })?;
            file_data = Some((filename, bytes.to_vec()));
            break;
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
        .unwrap_or("mp4");

    let temp_file = tempfile::Builder::new()
        .suffix(&format!(".{}", in_ext))
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

    let inspection = foldshift_media::inspect(temp_file.path()).map_err(|e| {
        let status = match e {
            foldshift_core::FcError::CapabilityUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::UNPROCESSABLE_ENTITY,
        };
        (status, Json(json!({ "error": e.to_string() })))
    })?;

    let mut modified = inspection;
    modified.path = PathBuf::from(filename);

    Ok(Json(modified))
}

async fn api_video_convert(
    mut multipart: Multipart,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let mut file_data: Option<(String, Vec<u8>)> = None;
    let mut target_format = String::from("mp4");
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;
    let mut fps: Option<f32> = None;
    let mut crf: Option<u8> = None;
    let mut video_bitrate: Option<String> = None;
    let mut audio_bitrate: Option<String> = None;
    let mut mute_audio = false;
    let force = true;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or("input.mp4").to_string();
                let bytes = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": format!("Upload error: {}", e) })),
                    )
                })?;
                file_data = Some((filename, bytes.to_vec()));
            }
            "format" | "to" => {
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
            "fps" => {
                if let Some(val) = field
                    .text()
                    .await
                    .ok()
                    .and_then(|t| t.trim().parse::<f32>().ok())
                    .filter(|&v| v > 0.0)
                {
                    fps = Some(val);
                }
            }
            "crf" => {
                if let Some(val) = field
                    .text()
                    .await
                    .ok()
                    .and_then(|t| t.trim().parse::<u8>().ok())
                {
                    crf = Some(val);
                }
            }
            "video_bitrate" => {
                if let Ok(text) = field.text().await {
                    let trimmed = text.trim().to_lowercase();
                    if !trimmed.is_empty() {
                        video_bitrate = Some(trimmed);
                    }
                }
            }
            "audio_bitrate" => {
                if let Ok(text) = field.text().await {
                    let trimmed = text.trim().to_lowercase();
                    if !trimmed.is_empty() {
                        audio_bitrate = Some(trimmed);
                    }
                }
            }
            "mute_audio" => {
                if let Ok(text) = field.text().await {
                    let trimmed = text.trim().to_lowercase();
                    mute_audio = trimmed == "true" || trimmed == "1" || trimmed == "yes";
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
        .unwrap_or("mp4");

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

    let resolution = match (width, height) {
        (Some(w), Some(h)) => Some((w, h)),
        _ => None,
    };

    let options = foldshift_media::VideoConvertOptions {
        resolution,
        width,
        height,
        fps,
        crf,
        video_bitrate,
        audio_bitrate,
        mute_audio,
        video_codec: None,
        force,
    };

    foldshift_media::convert_video(temp_input.path(), &output_path, options).map_err(|e| {
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
        .unwrap_or("output");
    let download_name = format!("{}.{}", stem, target_format);

    let mime_type = match target_format.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "wmv" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
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

async fn api_video_extract_audio(
    mut multipart: Multipart,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let mut file_data: Option<(String, Vec<u8>)> = None;
    let mut target_format = String::from("mp3");
    let force = true;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "file" => {
                let filename = field.file_name().unwrap_or("input.mp4").to_string();
                let bytes = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": format!("Upload error: {}", e) })),
                    )
                })?;
                file_data = Some((filename, bytes.to_vec()));
            }
            "format" | "to" => {
                if let Ok(text) = field.text().await {
                    let trimmed = text.trim().to_lowercase();
                    if !trimmed.is_empty() {
                        target_format = trimmed;
                    }
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
        .unwrap_or("mp4");

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

    foldshift_media::extract_audio(temp_input.path(), &output_path, force).map_err(|e| {
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
        .unwrap_or("output");
    let download_name = format!("{}.{}", stem, target_format);

    let mime_type = match target_format.as_str() {
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "opus" => "audio/opus",
        "wma" => "audio/x-ms-wma",
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
