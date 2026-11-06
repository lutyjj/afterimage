//! Rich text's Markdown as the blocks the Typst library draws: paragraphs of
//! runs, and lists whose items hold blocks. Headings become strong paragraphs;
//! links, code and images keep only their text; raw HTML keeps the text between
//! its tags. The library builds content from these values and never evaluates
//! the text as Typst markup.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use serde_json::{Map, Value, json};

/// Rich text longer than this is cut, at a character boundary: far more than a
/// screen holds, and a bound on the layout work a data source can ask for.
const MAX_TEXT: usize = 16 * 1024;

/// Replaces the `text` of every `richText` in a resolved screen with its
/// `blocks`.
pub fn prepare(screen: &mut Value) {
    match screen {
        Value::Object(object) => {
            if let Some(Value::Object(rich)) = object.get_mut("richText") {
                let text = match rich.remove("text") {
                    Some(Value::String(text)) => text,
                    None | Some(Value::Null) => String::new(),
                    Some(other) => other.to_string(),
                };
                rich.insert("blocks".to_owned(), blocks(&text));
            }
            object.values_mut().for_each(prepare);
        }
        Value::Array(items) => items.iter_mut().for_each(prepare),
        _ => {}
    }
}

/// A list being built, and the items it has.
struct List {
    start: Option<u64>,
    items: Vec<Value>,
}

#[derive(Default)]
struct Builder {
    /// The blocks of the document, then of each item being built.
    levels: Vec<Vec<Value>>,
    lists: Vec<List>,
    /// The runs of the paragraph being built.
    runs: Option<Vec<Value>>,
    strong: usize,
    emphasis: usize,
    heading: bool,
}

impl Builder {
    fn run(&mut self, text: &str) {
        let run = json!({
            "text": text,
            "strong": self.strong > 0 || self.heading,
            "emphasis": self.emphasis > 0,
        });
        self.runs.get_or_insert_with(Vec::new).push(run);
    }

    fn line_break(&mut self) {
        self.runs
            .get_or_insert_with(Vec::new)
            .push(json!({ "break": true }));
    }

    /// Ends the paragraph being built, if there is one.
    fn paragraph(&mut self) {
        if let Some(runs) = self.runs.take() {
            self.block(json!({ "paragraph": runs }));
        }
    }

    fn block(&mut self, block: Value) {
        if let Some(level) = self.levels.last_mut() {
            level.push(block);
        }
    }
}

/// The blocks of `markdown`.
pub fn blocks(markdown: &str) -> Value {
    let cut = markdown
        .char_indices()
        .nth(MAX_TEXT)
        .map_or(markdown, |(at, _)| &markdown[..at]);
    let mut builder = Builder {
        levels: vec![Vec::new()],
        ..Builder::default()
    };
    for event in Parser::new(cut) {
        match event {
            Event::Text(text) | Event::Code(text) => builder.run(&text),
            Event::SoftBreak => builder.run(" "),
            Event::HardBreak => builder.line_break(),
            Event::Start(Tag::Strong) => builder.strong += 1,
            Event::End(TagEnd::Strong) => builder.strong = builder.strong.saturating_sub(1),
            Event::Start(Tag::Emphasis) => builder.emphasis += 1,
            Event::End(TagEnd::Emphasis) => {
                builder.emphasis = builder.emphasis.saturating_sub(1);
            }
            Event::Start(Tag::Heading { .. }) => {
                builder.paragraph();
                builder.heading = true;
            }
            Event::End(TagEnd::Heading(_)) => {
                builder.paragraph();
                builder.heading = false;
            }
            Event::End(TagEnd::Paragraph | TagEnd::CodeBlock | TagEnd::BlockQuote(_)) => {
                builder.paragraph();
            }
            Event::Start(Tag::List(start)) => {
                builder.paragraph();
                builder.lists.push(List {
                    start,
                    items: Vec::new(),
                });
            }
            Event::Start(Tag::Item) => {
                builder.paragraph();
                builder.levels.push(Vec::new());
            }
            Event::End(TagEnd::Item) => {
                builder.paragraph();
                if let (Some(item), Some(list)) = (builder.levels.pop(), builder.lists.last_mut()) {
                    list.items.push(Value::Array(item));
                }
            }
            Event::End(TagEnd::List(_)) => {
                if let Some(list) = builder.lists.pop() {
                    let mut shape = Map::new();
                    shape.insert("items".to_owned(), Value::Array(list.items));
                    if let Some(start) = list.start {
                        shape.insert("start".to_owned(), json!(start));
                    }
                    builder.block(json!({ "list": shape }));
                }
            }
            _ => {}
        }
    }
    builder.paragraph();
    Value::Array(builder.levels.swap_remove(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, strong: bool, emphasis: bool) -> Value {
        json!({ "text": text, "strong": strong, "emphasis": emphasis })
    }

    #[test]
    fn paragraphs_hold_runs_of_strong_and_emphasised_text() {
        let blocks = blocks("First **bold** and *slanted*\nline.\n\nSecond  \nbroken.");
        assert_eq!(
            blocks,
            json!([
                { "paragraph": [
                    run("First ", false, false),
                    run("bold", true, false),
                    run(" and ", false, false),
                    run("slanted", false, true),
                    run(" ", false, false),
                    run("line.", false, false),
                ] },
                { "paragraph": [
                    run("Second", false, false),
                    { "break": true },
                    run("broken.", false, false),
                ] },
            ])
        );
    }

    #[test]
    fn lists_hold_their_items_and_where_numbering_starts() {
        let blocks =
            blocks("3. three\n4. four\n   - inner\n\n# Title\n[link](https://example.test) `code`");
        assert_eq!(
            blocks,
            json!([
                { "list": { "start": 3, "items": [
                    [{ "paragraph": [run("three", false, false)] }],
                    [
                        { "paragraph": [run("four", false, false)] },
                        { "list": { "items": [[{ "paragraph": [run("inner", false, false)] }]] } },
                    ],
                ] } },
                { "paragraph": [run("Title", true, false)] },
                { "paragraph": [run("link", false, false), run(" ", false, false), run("code", false, false)] },
            ])
        );
    }

    #[test]
    fn html_keeps_its_text_and_a_rich_text_gets_blocks() {
        let mut screen = json!({ "content": { "column": { "children": [
            { "richText": { "text": "<b>Hi</b> there", "size": "large" } },
            { "richText": { "text": 42 } }
        ] } } });
        prepare(&mut screen);
        let children = &screen["content"]["column"]["children"];
        assert_eq!(
            children[0]["richText"],
            json!({ "size": "large", "blocks": [{ "paragraph": [run("Hi", false, false), run(" there", false, false)] }] })
        );
        assert_eq!(
            children[1]["richText"]["blocks"],
            json!([{ "paragraph": [run("42", false, false)] }])
        );
    }

    #[test]
    fn text_past_the_bound_is_cut() {
        let long = "a".repeat(MAX_TEXT + 10);
        let blocks = blocks(&long);
        assert_eq!(
            blocks[0]["paragraph"][0]["text"].as_str().unwrap().len(),
            MAX_TEXT
        );
    }
}
