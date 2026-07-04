use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use image::ImageError;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    ImageFetchError(String),
    ImageProcessingError(ImageError),
    MultipartError(axum::extract::multipart::MultipartError),
    IoError(std::io::Error),
    ReqwestError(reqwest::Error),
    MissingImageFile,
    UnsupportedFilter(String),
    InvalidFilterParameters(String),
    UnsupportedOutputFormat(String),
    InvalidCropDimensions(&'static str),
    InvalidResizeDimensions(&'static str),
}

impl From<ImageError> for AppError {
    fn from(err: ImageError) -> Self {
        AppError::ImageProcessingError(err)
    }
}

impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(err: axum::extract::multipart::MultipartError) -> Self {
        AppError::MultipartError(err)
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::IoError(err)
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::ReqwestError(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::ImageFetchError(msg) => (
                StatusCode::BAD_REQUEST,
                format!("failed to fetch image: {}", msg),
            ),
            AppError::ImageProcessingError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("image processing failed: {}", err),
            ),
            AppError::MultipartError(err) => (
                StatusCode::BAD_REQUEST,
                format!("invalid multipart data: {}", err),
            ),
            AppError::IoError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("io error: {}", err),
            ),
            AppError::ReqwestError(err) => (
                StatusCode::BAD_GATEWAY,
                format!("external request failed: {}", err),
            ),
            AppError::MissingImageFile => (
                StatusCode::BAD_REQUEST,
                "no image file found in upload.".to_string(),
            ),
            AppError::UnsupportedFilter(filter) => (
                StatusCode::BAD_REQUEST,
                format!("unsupported filter type: {}", filter),
            ),
            AppError::InvalidFilterParameters(msg) => (
                StatusCode::BAD_REQUEST,
                format!("invalid filter parameters: {}", msg),
            ),
            AppError::UnsupportedOutputFormat(format) => (
                StatusCode::BAD_REQUEST,
                format!("unsupported output format: {}", format),
            ),
            AppError::InvalidCropDimensions(msg) => (StatusCode::BAD_REQUEST, msg.to_string()),
            AppError::InvalidResizeDimensions(msg) => (StatusCode::BAD_REQUEST, msg.to_string()),
        };

        let body = Json(json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}
