//! The render operation in Typst: a resolved screen, its images and a display's
//! model and palette in, the image the display shows out.
//!
//! A resolved screen is the model's `ScreenDocument` after the server has replaced
//! every template with its value and expanded every `each`, as JSON. The Typst
//! library in `src/typst/` draws each component from the design tokens that
//! `tools/tokens` resolves from TRMNL's design system for the display's profile
//! (`gen/design/tokens.json`, which `tools/generate` writes), with the fonts of
//! that release (`gen/design/fonts/`).
//! Images arrive as bytes under the keys the screen's `url` members name.

mod markdown;
mod raster;
mod tokens;

use std::collections::BTreeMap;

use include_dir::{Dir, include_dir};
use afterimage_trmnl::catalog::{Model, Palette};
use typst::foundations::{Array, Bytes, Dict, IntoValue, Str};
use typst::layout::{Abs, Frame, FrameItem, Point};
use typst::text::{Font, FontVariations};
use typst_as_lib::{TypstAsLibError, TypstEngine};
use typst_layout::PagedDocument;

use crate::tokens::Tokens;

static FONTS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../gen/design/fonts");
const MAIN: &str = include_str!("typst/main.typ");
const LIBRARY: &str = include_str!("typst/afterimage.typ");

/// An encoded image for a display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub bytes: Vec<u8>,
    /// Such as `image/png`.
    pub content_type: &'static str,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{model} takes {mime} images, which this renderer does not write")]
    UnsupportedFormat { model: String, mime: String },
    #[error("there are no design tokens for {model} with {palette}")]
    NoTokens { model: String, palette: String },
    #[error("the screen could not be drawn: {0}")]
    Typst(String),
    #[error(transparent)]
    Encode(#[from] raster::EncodeError),
    #[error("the image is {size} bytes, more than the {limit} the display takes")]
    TooLarge { size: usize, limit: u64 },
}

/// Compiles screens with the embedded Typst library and fonts. One renderer
/// serves every display; it is cheap to share and safe to use from many threads.
pub struct Renderer {
    engine: TypstEngine,
    /// Each font family's ascent and descent in ems, which the library sets
    /// CSS's line boxes with.
    metrics: Dict,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    #[must_use]
    pub fn new() -> Self {
        let fonts: Vec<&'static [u8]> = FONTS
            .files()
            .filter(|file| {
                file.path()
                    .extension()
                    .is_some_and(|extension| extension == "ttf")
            })
            .map(include_dir::File::contents)
            .collect();
        let mut metrics = Dict::new();
        for font in fonts.iter().flat_map(|data| Font::iter(Bytes::new(*data))) {
            let family = Str::from(font.info().family.as_str());
            if metrics.get(&family).is_err() {
                // Vertical metrics, which no axis of these fonts varies.
                let variant = font.info().variant;
                let instance = font.instantiate(variant, Abs::pt(16.0), &FontVariations::default());
                let em = instance.metrics();
                let sizes: Array = [em.ascender.get(), -em.descender.get()]
                    .into_iter()
                    .map(IntoValue::into_value)
                    .collect();
                metrics.insert(family, sizes.into_value());
            }
        }
        let engine = TypstEngine::builder()
            .fonts(fonts)
            .with_static_source_file_resolver([("main.typ", MAIN), ("afterimage.typ", LIBRARY)])
            .build();
        Self { engine, metrics }
    }

    /// Whether this renderer can draw for `model` showing `palette`: it writes the
    /// model's image format, and the profile has design tokens.
    #[must_use]
    pub fn supports(model: &Model, palette: &Palette) -> bool {
        model.mime_type == "image/png" && Tokens::get().of(&model.name, &palette.id).is_some()
    }

    /// Draws `screen` for a display of `model` showing `palette`.
    ///
    /// # Errors
    ///
    /// When the model takes a format this renderer does not write, the profile
    /// has no tokens, Typst rejects the screen, or the image exceeds the model's
    /// size limit.
    pub fn render(
        &self,
        screen: &serde_json::Value,
        images: &BTreeMap<String, Vec<u8>>,
        model: &Model,
        palette: &Palette,
    ) -> Result<Image, Error> {
        if model.mime_type != "image/png" {
            return Err(Error::UnsupportedFormat {
                model: model.name.clone(),
                mime: model.mime_type.clone(),
            });
        }
        let tokens = Tokens::get()
            .of(&model.name, &palette.id)
            .ok_or_else(|| Error::NoTokens {
                model: model.name.clone(),
                palette: palette.id.clone(),
            })?;
        let mut files = Dict::new();
        for (key, bytes) in images {
            files.insert(
                Str::from(key.as_str()),
                Bytes::new(bytes.clone()).into_value(),
            );
        }
        let mut inputs = Dict::new();
        let mut screen = screen.clone();
        markdown::prepare(&mut screen);
        inputs.insert("screen".into(), Str::from(screen.to_string()).into_value());
        inputs.insert("tokens".into(), Str::from(tokens).into_value());
        inputs.insert("images".into(), files.into_value());
        inputs.insert("metrics".into(), self.metrics.clone().into_value());
        // Gray text needs a gray to land on: a full-color palette, or one that lists
        // a gray between black and white. Elsewhere it is black.
        let grays = palette.rgb().is_none_or(|colors| {
            colors
                .iter()
                .any(|&[r, g, b]| r == g && g == b && r != 0 && r != u8::MAX)
        });
        inputs.insert("grays".into(), grays.into_value());
        inputs.insert(
            "width".into(),
            (f64::from(model.width) / model.scale_factor).into_value(),
        );
        inputs.insert(
            "height".into(),
            (f64::from(model.height) / model.scale_factor).into_value(),
        );
        let document: PagedDocument = self
            .engine
            .compile_with_input("main.typ", inputs)
            .output
            .map_err(|error| Error::Typst(describe(&error)))?;
        let page = document
            .pages()
            .first()
            .ok_or_else(|| Error::Typst("the screen has no page".to_owned()))?;
        let options = typst_render::RenderOptions {
            pixel_per_pt: model.scale_factor.into(),
            render_bleed: false,
        };
        let mut page = page.clone();
        page.frame = snapped(&page.frame, Point::zero(), model.scale_factor);
        let pixmap = typst_render::render(&page, &options);
        let colors = palette.rgb();
        let bytes = raster::encode(&pixmap, model.rotation, colors.as_deref())?;
        if let Some(limit) = model.image_size_limit
            && bytes.len() as u64 > limit
        {
            return Err(Error::TooLarge {
                size: bytes.len(),
                limit,
            });
        }
        Ok(Image {
            bytes,
            content_type: "image/png",
        })
    }
}

/// Typst's diagnostics, as one line each.
fn describe(error: &TypstAsLibError) -> String {
    match error {
        TypstAsLibError::TypstSource(diagnostics) => diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.to_string())
            .collect::<Vec<_>>()
            .join("; "),
        other => other.to_string(),
    }
}

/// `frame` with every text run's origin, the start of its baseline, moved to the
/// nearest whole device pixel at `scale` device pixels per point, as browsers
/// place text, so glyphs drawn on a pixel grid, such as TRMNL's pixel fonts, stay
/// on it instead of blurring across two pixels. The shapes beside the text in its
/// frame, a line's underline among them, move down or up with it, so they keep
/// their distance from its baseline. `origin` is the frame's place on the page;
/// inside a group that scales or skews, positions stay as laid out.
fn snapped(frame: &Frame, origin: Point, scale: f64) -> Frame {
    let nudge = |at: Abs| Abs::pt((at.to_pt() * scale).round() / scale) - at;
    // The text of one frame is one line, on one baseline.
    let line = frame.items().find_map(|(position, item)| {
        matches!(item, FrameItem::Text(_)).then(|| nudge(origin.y + position.y))
    });
    let mut out = Frame::new(frame.size(), frame.kind());
    if frame.has_baseline() {
        out.set_baseline(frame.baseline());
    }
    for (position, item) in frame.items() {
        let at = origin + *position;
        match item {
            FrameItem::Group(group) => {
                let transform = group.transform;
                let mut group = group.clone();
                let translates_only = transform.sx.is_one()
                    && transform.sy.is_one()
                    && transform.kx.is_zero()
                    && transform.ky.is_zero();
                if translates_only {
                    group.frame = snapped(
                        &group.frame,
                        at + Point::new(transform.tx, transform.ty),
                        scale,
                    );
                }
                out.push(*position, FrameItem::Group(group));
            }
            FrameItem::Text(_) => {
                out.push(
                    *position + Point::new(nudge(at.x), nudge(at.y)),
                    item.clone(),
                );
            }
            FrameItem::Shape(..) => {
                out.push(
                    *position + Point::with_y(line.unwrap_or_default()),
                    item.clone(),
                );
            }
            _ => out.push(*position, item.clone()),
        }
    }
    out
}
