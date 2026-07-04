use bytes::Bytes;
use image::{DynamicImage, ImageFormat, codecs::jpeg::JpegEncoder, imageops};
use std::io::Cursor;

use crate::error::AppError;

pub struct ProcessedImage {
    pub bytes: Vec<u8>,
    pub mime_type: String,
}

pub async fn fetch_image_bytes_from_url(url: &str) -> Result<Bytes, AppError> {
    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        return Err(AppError::ImageFetchError(format!(
            "failed to fetch image: server responded with {}",
            response.status()
        )));
    }
    let bytes = response.bytes().await?;
    Ok(bytes)
}

pub fn resize_image(
    img: DynamicImage,
    nwidth: u32,
    nheight: u32,
    filter: imageops::FilterType,
) -> DynamicImage {
    img.resize_exact(nwidth, nheight, filter)
}

pub fn crop_image(
    img: DynamicImage,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<DynamicImage, AppError> {
    if x.saturating_add(width) > img.width() || y.saturating_add(height) > img.height() {
        return Err(AppError::InvalidCropDimensions(
            "crop window is outside the image bounds.",
        ));
    }
    Ok(img.crop_imm(x, y, width, height))
}

pub fn apply_filter_str(img: DynamicImage, filter_str: &str) -> Result<DynamicImage, AppError> {
    let parts: Vec<&str> = filter_str.split(':').collect();
    let filter_name = parts[0].to_lowercase();

    match filter_name.as_str() {
        "grayscale" => Ok(img.grayscale()),
        "invert" => {
            let mut mutable_img = img;
            imageops::invert(&mut mutable_img);
            Ok(mutable_img)
        }
        "blur" => {
            let sigma = if parts.len() > 1 {
                parts[1].parse::<f32>().map_err(|_| {
                    AppError::InvalidFilterParameters("invalid blur sigma value".to_string())
                })?
            } else {
                1.0 // Default sigma
            };
            Ok(img.blur(sigma))
        }
        "sharpen" => {
            let sigma = if parts.len() > 1 {
                parts[1].parse::<f32>().map_err(|_| {
                    AppError::InvalidFilterParameters("invalid sharpen sigma value".to_string())
                })?
            } else {
                1.0 // Default sigma
            };
            let threshold = if parts.len() > 2 {
                parts[2].parse::<i32>().map_err(|_| {
                    AppError::InvalidFilterParameters("invalid sharpen threshold value".to_string())
                })?
            } else {
                // A common default threshold for unsharpen mask, may need tuning
                0
            };
            // Convert to Rgba8 buffer, apply unsharpen, then convert back to DynamicImage
            let rgba8 = img.to_rgba8();
            let result = imageops::unsharpen(&rgba8, sigma, threshold);
            Ok(image::DynamicImage::ImageRgba8(result))
        }
        // Example: "brighten:10"
        "brighten" => {
            let value = if parts.len() > 1 {
                parts[1].trim().parse::<i32>().map_err(|_| {
                    AppError::InvalidFilterParameters("invalid brighten value.".to_string())
                })?
            } else {
                10 // Default brighten value
            };
            Ok(img.brighten(value))
        }
        // Example: "contrast:15.5"
        "contrast" => {
            let value = if parts.len() > 1 {
                parts[1].trim().parse::<f32>().map_err(|_| {
                    AppError::InvalidFilterParameters("invalid contrast value.".to_string())
                })?
            } else {
                10.0 // Default contrast value
            };
            Ok(img.adjust_contrast(value))
        }
        // Add more filters here
        _ => Err(AppError::UnsupportedFilter(filter_name)),
    }
}

pub fn encode_image_to_bytes(
    img: DynamicImage,
    format_str: &str,
    quality: Option<u8>,
) -> Result<ProcessedImage, AppError> {
    let mut buffer = Cursor::new(Vec::new());

    match format_str.to_lowercase().as_str() {
        "png" => {
            img.write_to(&mut buffer, ImageFormat::Png)?;
            Ok(ProcessedImage {
                bytes: buffer.into_inner(),
                mime_type: "image/png".to_string(),
            })
        }
        "jpeg" | "jpg" => {
            let quality = quality.unwrap_or(80).max(1).min(100);
            img.write_with_encoder(JpegEncoder::new_with_quality(&mut buffer, quality))?;
            Ok(ProcessedImage {
                bytes: buffer.into_inner(),
                mime_type: "image/jpeg".to_string(),
            })
        }
        "webp" => {
            img.write_to(&mut buffer, ImageFormat::WebP)?;
            Ok(ProcessedImage {
                bytes: buffer.into_inner(),
                mime_type: "image/webp".to_string(),
            })
        }
        "bmp" => {
            img.write_to(&mut buffer, ImageFormat::Bmp)?;
            Ok(ProcessedImage {
                bytes: buffer.into_inner(),
                mime_type: "image/bmp".to_string(),
            })
        }
        "gif" => {
            img.write_to(&mut buffer, ImageFormat::Gif)?;
            Ok(ProcessedImage {
                bytes: buffer.into_inner(),
                mime_type: "image/gif".to_string(),
            })
        }
        _ => Err(AppError::UnsupportedOutputFormat(format_str.to_string())),
    }
}
