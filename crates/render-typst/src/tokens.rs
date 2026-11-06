//! The design tokens `tools/tokens` resolves from TRMNL's design system, one set
//! per display profile.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Document {
    /// `model/palette` to an index into `sets`.
    profiles: BTreeMap<String, usize>,
    sets: Vec<BTreeMap<String, Value>>,
    /// The dither tiles, as SVG text, that paints name by index.
    tiles: Vec<String>,
}

/// Each set as the JSON text the Typst library reads, its paints holding their
/// tiles' SVG text.
pub struct Tokens {
    profiles: BTreeMap<String, usize>,
    sets: Vec<String>,
}

static TOKENS: LazyLock<Tokens> = LazyLock::new(|| {
    let document: Document = serde_json::from_str(include_str!("../design/tokens.json"))
        .expect("the generated tokens parse");
    let sets = document
        .sets
        .into_iter()
        .map(|mut set| {
            for paint in set.values_mut().filter_map(Value::as_object_mut) {
                if let Some(index) = paint.get("tile").and_then(Value::as_u64) {
                    let tile = usize::try_from(index)
                        .ok()
                        .and_then(|index| document.tiles.get(index))
                        .expect("a paint names a tile the tokens hold");
                    paint.insert("tile".to_owned(), Value::String(tile.clone()));
                }
            }
            serde_json::to_string(&set).expect("tokens serialize")
        })
        .collect();
    Tokens {
        profiles: document.profiles,
        sets,
    }
});

impl Tokens {
    pub fn get() -> &'static Self {
        &TOKENS
    }

    /// The tokens of a model shown with a palette.
    pub fn of(&self, model: &str, palette: &str) -> Option<&str> {
        let index = *self.profiles.get(&format!("{model}/{palette}"))?;
        self.sets.get(index).map(String::as_str)
    }
}
