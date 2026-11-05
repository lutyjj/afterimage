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
}

/// Each set as the JSON text the Typst library reads.
pub struct Tokens {
    profiles: BTreeMap<String, usize>,
    sets: Vec<String>,
}

static TOKENS: LazyLock<Tokens> = LazyLock::new(|| {
    let document: Document = serde_json::from_str(include_str!("../design/tokens.json"))
        .expect("the generated tokens parse");
    Tokens {
        profiles: document.profiles,
        sets: document
            .sets
            .iter()
            .map(|set| serde_json::to_string(set).expect("tokens serialize"))
            .collect(),
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
