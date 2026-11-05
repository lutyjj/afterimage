//! TRMNL's catalogue of display models and palettes, from the snapshot in
//! `catalog/` that `tools/trmnl-catalog` refreshes from `GET /api/models` and
//! `GET /api/palettes`.

use std::sync::LazyLock;

use serde::Deserialize;

/// A display model as TRMNL publishes it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Model {
    /// Such as `og_plus`.
    pub name: String,
    pub label: String,
    /// Pixels.
    pub width: u32,
    pub height: u32,
    pub bit_depth: u32,
    /// Device pixels per layout pixel: TRMNL lays a screen out at `width /
    /// scale_factor` by `height / scale_factor` and draws it at full resolution.
    pub scale_factor: f64,
    /// Degrees the image is turned before it is sent.
    pub rotation: u32,
    /// Such as `image/png`.
    pub mime_type: String,
    /// `trmnl` for TRMNL's own hardware; `byod`, `kindle` and others for the rest.
    pub kind: String,
    /// Palettes the model can show, in TRMNL's order.
    pub palette_ids: Vec<String>,
    /// Largest image the device accepts, in bytes.
    pub image_size_limit: Option<u64>,
}

/// A palette as TRMNL publishes it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Palette {
    /// Such as `gray-4`.
    pub id: String,
    pub name: String,
    /// Levels of gray, for a gray palette.
    pub grays: u32,
    /// `#RRGGBB` colors, for a palette of fixed colors.
    #[serde(default)]
    pub colors: Option<Vec<String>>,
}

impl Palette {
    /// The palette's colors as RGB triples: its fixed colors, or its grays spread
    /// evenly from black to white. `None` for a palette of more colors than an image
    /// format needs to be told about (12-bit and 24-bit color).
    ///
    /// TRMNL's list gives `bw` and the full-color palettes the same fields (two
    /// grays, no colors), so only the id tells a gray palette from those.
    #[must_use]
    pub fn rgb(&self) -> Option<Vec<[u8; 3]>> {
        match &self.colors {
            Some(colors) if !colors.is_empty() => colors.iter().map(|hex| parse_hex(hex)).collect(),
            _ if self.id.starts_with("gray") || self.id == "bw" => {
                let levels = self.grays.max(2);
                Some(
                    (0..levels)
                        .map(|level| {
                            let value = u8::try_from(level * 255 / (levels - 1)).unwrap_or(u8::MAX);
                            [value; 3]
                        })
                        .collect(),
                )
            }
            _ => None,
        }
    }
}

fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    let digits = hex.strip_prefix('#')?;
    if digits.len() != 6 {
        return None;
    }
    let channel = |index: usize| u8::from_str_radix(digits.get(index..index + 2)?, 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?])
}

#[derive(Deserialize)]
struct List<T> {
    data: Vec<T>,
}

/// Both lists from the snapshot.
pub struct Catalog {
    pub models: Vec<Model>,
    pub palettes: Vec<Palette>,
}

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| Catalog {
    models: serde_json::from_str::<List<Model>>(include_str!("../catalog/models.json"))
        .expect("the model snapshot parses")
        .data,
    palettes: serde_json::from_str::<List<Palette>>(include_str!("../catalog/palettes.json"))
        .expect("the palette snapshot parses")
        .data,
});

impl Catalog {
    #[must_use]
    pub fn get() -> &'static Self {
        &CATALOG
    }

    #[must_use]
    pub fn model(&self, name: &str) -> Option<&Model> {
        self.models.iter().find(|model| model.name == name)
    }

    #[must_use]
    pub fn palette(&self, id: &str) -> Option<&Palette> {
        self.palettes.iter().find(|palette| palette.id == id)
    }

    /// Whether `palette` is one `model` supports.
    #[must_use]
    pub fn supports(&self, model: &str, palette: &str) -> bool {
        self.model(model)
            .is_some_and(|model| model.palette_ids.iter().any(|id| id == palette))
    }

    /// The display a newly claimed device starts with. A model whose name is the one
    /// the firmware reports wins. Otherwise the panel size decides: among models of
    /// that size, TRMNL's own hardware first, then the one with the most gray levels.
    /// A device of an unknown size gets the catalogue's most widely shown TRMNL
    /// display, its first `trmnl` model at the most common size, at its lowest depth.
    #[must_use]
    pub fn default_display(
        &self,
        firmware_model: &str,
        size: Option<(u32, u32)>,
    ) -> (&Model, &Palette) {
        let model = self
            .model(firmware_model)
            .or_else(|| {
                let (width, height) = size?;
                self.models
                    .iter()
                    .filter(|model| model.width == width && model.height == height)
                    .filter(|model| self.default_palette(model).rgb().is_some())
                    .max_by_key(|model| (model.kind == "trmnl", self.gray_levels(model)))
            })
            .unwrap_or_else(|| self.fallback_model());
        (model, self.default_palette(model))
    }

    /// The palette with as many levels as the model's bit depth gives, or else its
    /// first.
    #[must_use]
    pub fn default_palette(&self, model: &Model) -> &Palette {
        let levels = 1u32.checked_shl(model.bit_depth).unwrap_or(u32::MAX);
        let supported = || model.palette_ids.iter().filter_map(|id| self.palette(id));
        supported()
            .find(|palette| palette.colors.is_none() && palette.grays == levels)
            .or_else(|| supported().next())
            .unwrap_or(&self.palettes[0])
    }

    fn gray_levels(&self, model: &Model) -> u32 {
        let palette = self.default_palette(model);
        if palette.colors.is_none() {
            palette.grays
        } else {
            0
        }
    }

    fn fallback_model(&self) -> &Model {
        let trmnl = || self.models.iter().filter(|model| model.kind == "trmnl");
        let common = trmnl()
            .map(|model| (model.width, model.height))
            .max_by_key(|size| trmnl().filter(|m| (m.width, m.height) == *size).count());
        trmnl()
            .filter(|model| Some((model.width, model.height)) == common)
            .min_by_key(|model| model.bit_depth)
            .unwrap_or(&self.models[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_both_lists() {
        let catalog = Catalog::get();
        assert!(catalog.models.len() > 10);
        let model = catalog.model("og_plus").unwrap();
        assert_eq!((model.width, model.height, model.bit_depth), (800, 480, 2));
        assert!(catalog.supports("og_plus", "gray-4"));
        assert!(!catalog.supports("og_plus", "color-6a"));
    }

    #[test]
    fn spreads_grays_and_reads_fixed_colors() {
        let catalog = Catalog::get();
        assert_eq!(
            catalog.palette("gray-4").unwrap().rgb().unwrap(),
            [[0; 3], [85; 3], [170; 3], [255; 3]]
        );
        assert_eq!(
            catalog.palette("bw").unwrap().rgb().unwrap(),
            [[0; 3], [255; 3]]
        );
        let bwr = catalog.palette("color-3bwr").unwrap().rgb().unwrap();
        assert_eq!(bwr, [[0, 0, 0], [255, 0, 0], [255, 255, 255]]);
        assert_eq!(catalog.palette("color-24bit").unwrap().rgb(), None);
    }

    #[test]
    fn a_model_name_the_firmware_reports_wins() {
        let (model, palette) = Catalog::get().default_display("v2", Some((800, 480)));
        assert_eq!(model.name, "v2");
        assert_eq!(palette.id, "gray-16", "four bits give sixteen grays");
    }

    #[test]
    fn otherwise_the_panel_size_picks_trmnl_hardware_with_the_most_grays() {
        let catalog = Catalog::get();
        let (model, palette) = catalog.default_display("xiao_epaper_display", Some((800, 480)));
        assert_eq!(model.kind, "trmnl");
        assert_eq!((model.width, model.height), (800, 480));
        let most = catalog
            .models
            .iter()
            .filter(|m| m.kind == "trmnl" && (m.width, m.height) == (800, 480))
            .map(|m| catalog.gray_levels(m))
            .max()
            .unwrap();
        assert_eq!(palette.grays, most);
        assert!(palette.colors.is_none());
    }

    #[test]
    fn an_unknown_size_gets_a_trmnl_display_at_its_lowest_depth() {
        let catalog = Catalog::get();
        let (model, _) = catalog.default_display("unknown", None);
        assert_eq!(model.kind, "trmnl");
        let (same_size, _) = catalog.default_display("unknown", Some((1, 1)));
        assert_eq!(model, same_size);
        assert!(
            catalog
                .models
                .iter()
                .filter(|m| m.kind == "trmnl" && (m.width, m.height) == (model.width, model.height))
                .all(|m| m.bit_depth >= model.bit_depth)
        );
    }
}
