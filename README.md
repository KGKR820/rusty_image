# rusty_image

An async HTTP microservice for on-the-fly image manipulation — resize, crop, filter, and re-encode images, sourced either from a remote URL or a direct file upload. Built on [Axum](https://github.com/tokio-rs/axum), [Tokio](https://tokio.rs/), and the [`image`](https://github.com/image-rs/image) crate.

## Why this exists

Most image APIs force you to pick between "fetch from a URL" and "upload a file." This service supports both through the same transformation pipeline, so the same query parameters work whether the source image is already on the web or sitting on your disk.

## Tech stack

| Layer | Crate |
|---|---|
| Web framework | `axum` 0.8 |
| Async runtime | `tokio` |
| Image decoding/encoding | `image` |
| HTTP client (for URL fetches) | `reqwest` (rustls backend) |
| Logging | `tracing` + `tracing-subscriber` |

## Getting started

```bash
git clone https://github.com/KGKR820/rusty_image.git
cd rusty_image
cargo run
```

By default the server binds to `0.0.0.0:3000`. To see request logs while developing, set a log level:

```bash
RUST_LOG=rusty_image=debug cargo run
```

## Endpoints at a glance

| Method | Path | Source of image |
|---|---|---|
| `GET` | `/url` | Remote image, fetched via `url` query param |
| `POST` | `/upload` | Local file, sent as multipart form data |

Both endpoints accept the same set of transformation parameters — the only difference is how the source image gets in.

## Transformation parameters

| Param | Applies to | Notes |
|---|---|---|
| `w`, `h` | resize | Give both for exact dimensions, or just one to resize with aspect ratio preserved |
| `crop_x`, `crop_y`, `crop_w`, `crop_h` | crop | All four required together; crop always runs before resize |
| `filter` | effects | See filter table below |
| `output_format` | encoding | `png`, `jpeg`, `webp`, `bmp`, or `gif` |
| `quality` | encoding | 1–100, only affects JPEG output |

## Filters

| Name | Syntax | What it does |
|---|---|---|
| Grayscale | `grayscale` | Strips color |
| Invert | `invert` | Inverts all channel values |
| Blur | `blur:<sigma>` | Gaussian blur, e.g. `blur:4` |
| Sharpen | `sharpen:<sigma>:<threshold>` | Unsharp mask, e.g. `sharpen:1.5:3` |
| Brighten | `brighten:<value>` | Positive or negative integer offset |
| Contrast | `contrast:<value>` | Positive or negative float |

## Usage examples

**Resize + convert a remote image to WebP:**

```bash
curl "http://localhost:3000/url?url=https://picsum.photos/id/1015/1200/800&w=480&output_format=webp" -o mountain.webp
```

**Crop a square out of the center, then sharpen it:**

```bash
curl "http://localhost:3000/url?url=https://picsum.photos/id/1015/1200/800&crop_x=300&crop_y=200&crop_w=500&crop_h=500&filter=sharpen:2:4" -o cropped.png
```

**Upload a local file, resize to a fixed width, and drop the JPEG quality for a smaller file size:**

```bash
curl -X POST http://localhost:3000/upload \
  -F "image=@portrait.jpg" \
  -F "w=600" \
  -F "output_format=jpeg" \
  -F "quality=60" \
  -o portrait-small.jpg
```

**Quick browser test** — filters and resizing work directly in the address bar for `/url`:

```
http://localhost:3000/url?url=https://picsum.photos/id/1015/1200/800&filter=grayscale&w=600
```

## Project layout

```
src/
├── main.rs   # routes, request handlers, transformation pipeline
├── ops.rs    # resize/crop/filter/encode primitives
├── error.rs  # centralized AppError + conversions from axum/reqwest/image errors
└── lib.rs
```

