use std::path::Path;

use axum::{
    Router,
    extract::{DefaultBodyLimit, Multipart, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use bytes::Bytes;
use image::{DynamicImage, GenericImageView, imageops::FilterType};
use serde::Deserialize;
use tokio::net::TcpListener;
use tracing::debug;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use rusty_image::{
    error::AppError,
    ops::{self, ProcessedImage, apply_filter_str},
};

#[derive(Deserialize, Debug)]
struct ImageUrlParams {
    url: String,
    w: Option<u32>,
    h: Option<u32>,
    crop_x: Option<u32>,
    crop_y: Option<u32>,
    crop_w: Option<u32>,
    crop_h: Option<u32>,
    filter: Option<String>,
    output_format: Option<String>,
    quality: Option<u8>,
}

#[derive(Deserialize, Debug, Default)]
struct ImageFormDataParams {
    w: Option<u32>,
    h: Option<u32>,
    crop_x: Option<u32>,
    crop_y: Option<u32>,
    crop_w: Option<u32>,
    crop_h: Option<u32>,
    filter: Option<String>,
    output_format: Option<String>,
    quality: Option<u8>,
}

const MAX_UPLOAD_SIZE: usize = 10 * 1024 * 1024; // 10MB
const MAX_DIMENSION: u32 = 8192;

#[derive(Clone)]
struct AppState {
    client: reqwest::Client,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "rusty_image=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();

    let state = AppState { client };

    let app = Router::new()
        .route("/url", get(process_image_from_url))
        .route("/upload", post(process_image_from_upload))
        .with_state(state)
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_SIZE));

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();
    debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn process_image_from_url(
    State(state): State<AppState>,
    Query(params): Query<ImageUrlParams>,
) -> Result<impl IntoResponse, AppError> {
    tracing::debug!("Processing image from URL: {:?}", params);

    let image_bytes = ops::fetch_image_bytes_from_url(&state.client, &params.url).await?;
    let mut img = image::load_from_memory(&image_bytes)?;

    img = apply_transformations(
        img,
        params.w,
        params.h,
        params.crop_x,
        params.crop_y,
        params.crop_w,
        params.crop_h,
        params.filter,
    )?;

    let output_format_str = params
        .output_format
        .clone()
        .unwrap_or_else(|| infer_format_from_url_or_default(&params.url, "png"));

    let processed_image = ops::encode_image_to_bytes(img, &output_format_str, params.quality)?;

    let mut headers = HeaderMap::new();
    headers.insert(
        "Content-Type",
        HeaderValue::from_str(&processed_image.mime_type).unwrap(),
    );

    Ok((StatusCode::OK, headers, processed_image.bytes))
}

async fn process_image_from_upload(
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AppError> {
    debug!("Processing image from upload");

    let mut image_bytes: Option<Bytes> = None;
    let mut image_filename: Option<String> = None;
    let mut form_params = ImageFormDataParams::default();

    while let Some(field) = multipart.next_field().await? {
        let name = if let Some(name) = field.name() {
            name.to_string()
        } else {
            continue;
        };

        match name.as_str() {
            "image" => {
                if image_bytes.is_none() {
                    if let Some(file_name) = field.file_name() {
                        image_filename = Some(file_name.to_string());
                    }
                    image_bytes = Some(field.bytes().await?);
                }
            }
            "w" => form_params.w = field.text().await?.parse().ok(),
            "h" => form_params.h = field.text().await?.parse().ok(),
            "crop_x" => form_params.crop_x = field.text().await?.parse().ok(),
            "crop_y" => form_params.crop_y = field.text().await?.parse().ok(),
            "crop_w" => form_params.crop_w = field.text().await?.parse().ok(),
            "crop_h" => form_params.crop_h = field.text().await?.parse().ok(),
            "filter" => form_params.filter = Some(field.text().await?),
            "output_format" => form_params.output_format = Some(field.text().await?),
            "quality" => form_params.quality = field.text().await?.parse().ok(),
            _ => {
                // ignore
            }
        }
    }

    let image_bytes = image_bytes.ok_or_else(|| AppError::MissingImageFile)?;
    let mut img = image::load_from_memory(&image_bytes)?;

    debug!("Form params from upload: {:?}", form_params);

    img = apply_transformations(
        img,
        form_params.w,
        form_params.h,
        form_params.crop_x,
        form_params.crop_y,
        form_params.crop_w,
        form_params.crop_h,
        form_params.filter,
    )?;

    let output_format_str = form_params
        .output_format
        .unwrap_or_else(|| infer_format_from_filename_or_default(image_filename.as_deref(), "png"));

    let processed_image = ops::encode_image_to_bytes(img, &output_format_str, form_params.quality)?;

    send_image_response(processed_image)
}

fn send_image_response(processed_image: ProcessedImage) -> Result<impl IntoResponse, AppError> {
    let mut headers = HeaderMap::new();
    match HeaderValue::from_str(&processed_image.mime_type) {
        Ok(val) => {
            headers.insert("Content-Type", val);
        }
        Err(_) => {
            return Err(AppError::ImageProcessingError(
                image::ImageError::Unsupported(
                    image::error::UnsupportedError::from_format_and_kind(
                        image::error::ImageFormatHint::Unknown,
                        image::error::UnsupportedErrorKind::GenericFeature(
                            "invalid MIME type generated".to_string(),
                        ),
                    ),
                ),
            ));
        }
    }
    Ok((StatusCode::OK, headers, processed_image.bytes))
}

fn apply_transformations(
    mut img: DynamicImage,
    w: Option<u32>,
    h: Option<u32>,
    crop_x: Option<u32>,
    crop_y: Option<u32>,
    crop_w: Option<u32>,
    crop_h: Option<u32>,
    filter_str: Option<String>,
) -> Result<DynamicImage, AppError> {
    // Crop if all crop parameters are present
    if let (Some(cx), Some(cy), Some(cw), Some(ch)) = (crop_x, crop_y, crop_w, crop_h) {
        if cw > 0 && ch > 0 {
            if cw > MAX_DIMENSION || ch > MAX_DIMENSION {
                return Err(AppError::InvalidCropDimensions(
                    "crop dimensions exceed maximum allowed limit of 8192px.",
                ));
            }
            img = ops::crop_image(img, cx, cy, cw, ch)?;
        } else {
            return Err(AppError::InvalidCropDimensions(
                "crop width and height must be greater than 0.",
            ));
        }
    }

    // Resize if width or height is present
    let (current_w, current_h) = img.dimensions();
    let target_w = w.unwrap_or(current_w);
    let target_h = h.unwrap_or(current_h);

    if w.is_some() || h.is_some() {
        if target_w > MAX_DIMENSION || target_h > MAX_DIMENSION {
            return Err(AppError::InvalidResizeDimensions(
                "resize dimensions exceed maximum allowed limit of 8192px.",
            ));
        }
        if target_w > 0 && target_h > 0 {
            // If one dimension is not specified for resize, maintain aspect ratio
            let (final_w, final_h) = if w.is_none() && h.is_some() {
                // height specified, width auto
                let aspect_ratio = current_w as f32 / current_h as f32;
                ((target_h as f32 * aspect_ratio) as u32, target_h)
            } else if w.is_some() && h.is_none() {
                // width specified, height auto
                let aspect_ratio = current_h as f32 / current_w as f32;
                (target_w, (target_w as f32 * aspect_ratio) as u32)
            } else {
                // both specified or neither (no resize if neither)
                (target_w, target_h)
            };

            if final_w > 0 && final_h > 0 {
                img = ops::resize_image(img, final_w, final_h, FilterType::Triangle);
            } else if w.is_some() || h.is_some() {
                // only error if a resize was intended
                return Err(AppError::InvalidResizeDimensions(
                    "resize width and height must result in dimensions greater than 0",
                ));
            }
        } else if w.is_some() || h.is_some() {
            // only error if a resize was intended
            return Err(AppError::InvalidResizeDimensions(
                "target resize width and height must be greater than 0",
            ));
        }
    }

    // Apply filter if present
    if let Some(f_str) = filter_str {
        if !f_str.trim().is_empty() {
            img = apply_filter_str(img, &f_str)?;
        }
    }

    Ok(img)
}

fn infer_format_from_url_or_default(url: &str, default: &str) -> String {
    Path::new(url)
        .extension()
        .and_then(|os_str| os_str.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| default.to_string())
}

fn infer_format_from_filename_or_default(filename: Option<&str>, default: &str) -> String {
    filename
        .and_then(|f_name| {
            Path::new(f_name)
                .extension()
                .and_then(|os_str| os_str.to_str())
                .map(|s| s.to_lowercase())
        })
        .unwrap_or_else(|| default.to_string())
}
