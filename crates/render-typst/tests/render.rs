//! Screens drawn for real display profiles, read back from the encoded image.

use std::collections::BTreeMap;

use afterimage_render_typst::Renderer;
use afterimage_trmnl::catalog::Catalog;
use serde_json::{Value, json};

/// A resolved weather screen: every template already replaced by its value.
fn weather() -> Value {
    let hourly: Vec<f64> = (0..24)
        .map(|h| 6.0 + 4.0 * (f64::from(h) / 24.0 * std::f64::consts::TAU).sin())
        .collect();
    let hours: Vec<String> = (0..24).map(|h| format!("{h:02}")).collect();
    json!({
        "titleBar": { "title": "Weather", "instance": "Home" },
        "content": { "column": { "gap": "medium", "align": "start", "justify": "start", "grow": false, "children": [
            { "grid": { "columns": 4, "gap": "medium", "grow": false, "cells": [
                { "span": 1, "content": { "item": { "emphasis": 1, "plain": false, "grow": false, "children": [
                    { "value": { "text": "7.4°", "size": "xlarge", "fit": true, "tabular": false } },
                    { "label": { "text": "Partly cloudy", "size": "base", "style": "plain" } }
                ] } } },
                { "span": 1, "content": { "item": { "emphasis": 2, "plain": false, "grow": false, "children": [
                    { "value": { "text": "2–10°", "size": "large", "fit": false, "tabular": true } },
                    { "label": { "text": "Next 24 h", "size": "small", "style": "gray" } }
                ] } } },
                { "span": 1, "content": { "item": { "emphasis": 3, "plain": false, "grow": false, "children": [
                    { "value": { "text": "81%", "size": "large", "fit": false, "tabular": false } },
                    { "label": { "text": "Humidity", "size": "base", "style": "outline" } }
                ] } } },
                { "span": 1, "content": { "item": { "emphasis": 1, "plain": false, "index": "4", "grow": false, "children": [
                    { "value": { "text": "14 km/h", "size": "large", "fit": false, "tabular": false } },
                    { "label": { "text": "Wind", "size": "base", "style": "filled" } }
                ] } } }
            ] } },
            { "divider": {} },
            { "lineChart": { "grow": true, "series": [ { "values": hourly } ], "labels": hours } }
        ] } }
    })
}

fn render(screen: &Value, model: &str, palette: &str) -> Vec<u8> {
    let catalog = Catalog::get();
    let image = Renderer::new()
        .render(
            screen,
            &BTreeMap::new(),
            catalog.model(model).unwrap(),
            catalog.palette(palette).unwrap(),
        )
        .unwrap();
    assert_eq!(image.content_type, "image/png");
    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{model}.png"));
    std::fs::write(&out, &image.bytes).unwrap();
    image.bytes
}

fn info(bytes: &[u8]) -> png::Info<'static> {
    png::Decoder::new(std::io::Cursor::new(bytes.to_vec()))
        .read_info()
        .unwrap()
        .info()
        .clone()
}

#[test]
fn draws_a_two_bit_screen_at_the_panel_size() {
    let bytes = render(&weather(), "og_plus", "gray-4");
    let info = info(&bytes);
    assert_eq!((info.width, info.height), (800, 480));
    assert_eq!(
        (info.color_type, info.bit_depth),
        (png::ColorType::Indexed, png::BitDepth::Two)
    );
    assert_eq!(info.palette.unwrap().len(), 4 * 3);
}

#[test]
fn draws_one_bit_and_scaled_profiles() {
    let bytes = render(&weather(), "og_png", "bw");
    assert_eq!(info(&bytes).bit_depth, png::BitDepth::One);
    let bytes = render(&weather(), "v2", "gray-16");
    let info = info(&bytes);
    assert_eq!(
        (info.width, info.height, info.bit_depth),
        (1872, 1404, png::BitDepth::Four)
    );
}

/// Each pixel's palette color, row by row.
fn pixels(bytes: &[u8]) -> Vec<Vec<[u8; 3]>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes.to_vec()));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().unwrap();
    let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut buffer).unwrap();
    let channels = frame.color_type.samples();
    buffer[..frame.buffer_size()]
        .chunks(frame.line_size)
        .map(|line| {
            line.chunks(channels)
                .take(frame.width as usize)
                .map(|pixel| match pixel {
                    [luma] | [luma, _] => [*luma; 3],
                    [r, g, b, ..] => [*r, *g, *b],
                    [] => [255; 3],
                })
                .collect()
        })
        .collect()
}

/// Rows holding anything but white.
fn inked_rows(bytes: &[u8]) -> usize {
    pixels(bytes)
        .iter()
        .filter(|row| row.iter().any(|pixel| *pixel != [255, 255, 255]))
        .count()
}

fn screen_of(component: &Value) -> Value {
    json!({ "content": { "column": { "gap": "medium", "align": "start", "justify": "start", "grow": false, "children": [component] } } })
}

#[test]
fn gray_text_stays_visible_on_a_palette_without_grays() {
    let label = screen_of(
        &json!({ "label": { "text": "Updated 14:05", "size": "base", "style": "gray" } }),
    );
    for palette in ["color-6a", "bw", "gray-4"] {
        let model = if palette == "color-6a" {
            "inky_impression_7_3"
        } else {
            "og_plus"
        };
        let bytes = render(&label, model, palette);
        assert!(
            inked_rows(&bytes) > 5,
            "{model} {palette}: the label vanished"
        );
    }
}

#[test]
fn clamped_text_keeps_to_its_lines() {
    let long = "A description long enough to run over many lines of the screen, so that \
                cutting it to one line leaves most of it out and ends it with an ellipsis. "
        .repeat(8);
    let free = render(
        &screen_of(&json!({ "description": { "text": long, "size": "base" } })),
        "og_plus",
        "gray-4",
    );
    let clamped = render(
        &screen_of(&json!({ "description": { "text": long, "size": "base", "clamp": 1 } })),
        "og_plus",
        "gray-4",
    );
    let label = render(
        &screen_of(
            &json!({ "label": { "text": long, "size": "base", "style": "plain", "clamp": 1 } }),
        ),
        "og_plus",
        "gray-4",
    );
    let (free, clamped, label) = (inked_rows(&free), inked_rows(&clamped), inked_rows(&label));
    assert!(free > 4 * clamped, "free {free} rows, clamped {clamped}");
    assert!(
        label <= clamped + 4,
        "label {label} rows, description {clamped}"
    );
}

#[test]
fn clamped_text_in_a_fallback_font_keeps_what_fits() {
    // Cyrillic comes from the fallback font, whose lines are taller than the
    // description font's; a line of it must still count as one line.
    let long = "Сменить насадку на зубной щетке и поставить стирку до вечера. ".repeat(4);
    let bytes = render(
        &screen_of(&json!({ "description": { "text": long, "size": "base", "clamp": 1 } })),
        "og_plus",
        "gray-4",
    );
    let (rows, columns) = inked(&bytes);
    let width = columns.last().unwrap() - columns.first().unwrap();
    assert!(width > 300, "the clamped line is {width} pixels wide");
    assert!(rows.len() < 40, "{} inked rows for one line", rows.len());
}

#[test]
fn clamped_text_measures_its_tallest_characters_wherever_they_are() {
    // Non-ASCII characters the face may hold come first; Cyrillic, from the
    // fallback font, comes after them.
    let long = format!(
        "àáâäçèéêëìíîïñòóôöùúûü° {}",
        "Сменить насадку на зубной щетке и поставить стирку. ".repeat(4)
    );
    let bytes = render(
        &screen_of(&json!({ "description": { "text": long, "size": "base", "clamp": 1 } })),
        "og_plus",
        "gray-4",
    );
    let (rows, columns) = inked(&bytes);
    let width = columns.last().unwrap() - columns.first().unwrap();
    assert!(width > 300, "the clamped line is {width} pixels wide");
    assert!(rows.len() < 40, "{} inked rows for one line", rows.len());
}

#[test]
fn a_fitted_value_shrinks_into_its_box() {
    // Far wider than the first of four columns, a quarter of the 800-pixel screen.
    let screen = json!({ "content": { "grid": { "columns": 4, "gap": "medium", "grow": false, "cells": [
        { "span": 1, "content": { "value": { "text": "1234567890", "size": "peta", "fit": true, "tabular": false } } }
    ] } } });
    let bytes = render(&screen, "og_plus", "gray-4");
    let rightmost = pixels(&bytes)
        .iter()
        .filter_map(|row| row.iter().rposition(|pixel| *pixel != [255, 255, 255]))
        .max()
        .unwrap();
    assert!(rightmost < 200, "the value reaches x={rightmost}");
}

/// Rows and columns, top to bottom and left to right, that hold anything but white.
fn inked(bytes: &[u8]) -> (Vec<usize>, Vec<usize>) {
    let pixels = pixels(bytes);
    let ink = |pixel: &[u8; 3]| *pixel != [255, 255, 255];
    let rows = (0..pixels.len())
        .filter(|&y| pixels[y].iter().any(ink))
        .collect();
    let columns = (0..pixels[0].len())
        .filter(|&x| pixels.iter().any(|row| ink(&row[x])))
        .collect();
    (rows, columns)
}

fn title(text: &str) -> Value {
    json!({ "title": { "text": text, "size": "base" } })
}

fn stack(direction: &str, grow: bool, children: &[Value]) -> Value {
    json!({ direction: { "gap": "medium", "align": "start", "justify": "start", "grow": grow, "children": children } })
}

#[test]
fn a_row_that_does_not_grow_is_as_tall_as_its_content() {
    // A row above a title: the title stays on the screen, a line or two below.
    let screen = json!({ "content": stack("column", false, &[
        stack("row", false, &[title("Left"), title("Right")]),
        title("Below"),
    ]) });
    let (rows, _) = inked(&render(&screen, "og_plus", "gray-4"));
    let last = *rows.last().unwrap();
    assert!((40..120).contains(&last), "ink ends at y={last}");
}

#[test]
fn a_divider_or_a_chart_keeps_a_row_as_tall_as_its_content() {
    let chart = json!({ "lineChart": { "grow": true, "series": [ { "values": [1, 3, 2, 4] } ] } });
    for (name, inside) in [("divider", json!({ "divider": {} })), ("chart", chart)] {
        let screen = json!({ "content": stack("column", false, &[
            stack("row", false, &[title("Left"), inside, title("Right")]),
            title("Below"),
        ]) });
        let (rows, _) = inked(&render(&screen, "og_plus", "gray-4"));
        let last = *rows.last().unwrap();
        // A chart on a height sized to content is at most 200 pixels tall, and
        // "Below" follows it.
        assert!(last < 260, "{name}: ink ends at y={last}");
        assert!(rows.len() > 30, "{name}: {} inked rows", rows.len());
    }
}

#[test]
fn a_divider_in_a_row_spans_the_row() {
    // A divider between a one-line title and a three-line description reaches
    // as far down as the description does.
    let screen = json!({ "content": stack("column", false, &[
        stack("row", false, &[
            title("Left"),
            json!({ "divider": {} }),
            json!({ "description": { "text": "One\nTwo\nThree", "size": "base" } }),
        ]),
    ]) });
    let pixels = pixels(&render(&screen, "og_plus", "gray-4"));
    let ink = |pixel: &[u8; 3]| *pixel != [255, 255, 255];
    let (rows, _) = inked(&render(&screen, "og_plus", "gray-4"));
    let bottom = *rows.last().unwrap();
    // The divider is the one column inked on the description's last line and on
    // the title's first.
    let line = (0..pixels[0].len())
        .find(|&x| ink(&pixels[bottom][x]) && ink(&pixels[rows[0] + 2][x]) && x > 20)
        .map(|x| x < 200);
    assert_eq!(line, Some(true), "no divider spans the row");
}

/// The blank space before and after the divider's line along one axis: from the
/// end of the ink before it to the line, and from the line to the ink after it.
fn gaps_around(ink: &[usize], line: usize) -> (usize, usize) {
    let before = ink.iter().copied().filter(|&at| at < line).max().unwrap();
    let after = ink.iter().copied().filter(|&at| at > line).min().unwrap();
    (line - before, after - line)
}

#[test]
fn a_divider_sits_midway_between_its_neighbours() {
    // In a row: the line is the column holding the most ink.
    let row = json!({ "content": stack("column", false, &[
        stack("row", false, &[title("Left"), json!({ "divider": {} }), title("Right")]),
    ]) });
    let drawn = pixels(&render(&row, "og_plus", "gray-4"));
    let (_, columns) = inked(&render(&row, "og_plus", "gray-4"));
    let ink = |pixel: &[u8; 3]| *pixel != [255, 255, 255];
    let line = *columns
        .iter()
        .max_by_key(|&&x| drawn.iter().filter(|row| ink(&row[x])).count())
        .unwrap();
    let others: Vec<usize> = columns
        .iter()
        .copied()
        .filter(|&x| x.abs_diff(line) > 1)
        .collect();
    let (before, after) = gaps_around(&others, line);
    assert!(
        before.abs_diff(after) <= 3,
        "{before} px before the line, {after} after"
    );

    // In a column: the line is the row holding the most ink.
    let column = json!({ "content": stack("column", false, &[title("Above"), json!({ "divider": {} }), title("Below")]) });
    let drawn = pixels(&render(&column, "og_plus", "gray-4"));
    let (rows, _) = inked(&render(&column, "og_plus", "gray-4"));
    let line = *rows
        .iter()
        .max_by_key(|&&y| drawn[y].iter().filter(|pixel| ink(pixel)).count())
        .unwrap();
    let others: Vec<usize> = rows
        .iter()
        .copied()
        .filter(|&y| y.abs_diff(line) > 1)
        .collect();
    let (before, after) = gaps_around(&others, line);
    assert!(
        before.abs_diff(after) <= 3,
        "{before} px above the line, {after} below"
    );
}

#[test]
fn a_divider_in_an_item_keeps_the_item_as_wide_as_its_content() {
    // An item beside a title in a row that does not grow: the title follows the
    // item's widest line, not the far edge of the screen.
    let item = json!({ "item": { "emphasis": 1, "plain": false, "grow": false, "children": [
        title("Short"), { "divider": {} }, title("Text")
    ] } });
    let screen = json!({ "content": stack("column", false, &[
        stack("row", false, &[item, title("Right")]),
    ]) });
    let (_, columns) = inked(&render(&screen, "og_plus", "gray-4"));
    let last = *columns.last().unwrap();
    assert!(last < 300, "ink reaches x={last}");
}

#[test]
fn an_items_children_follow_the_alignment_around_it() {
    // A plain item in a column centred across the screen: its lines are centred.
    let item = json!({ "item": { "emphasis": 1, "plain": true, "grow": false, "children": [
        title("A"), title("A much longer line of text")
    ] } });
    let screen = json!({ "content": { "column": {
        "gap": "medium", "align": "center", "justify": "start", "grow": false, "children": [item]
    } } });
    let (_, columns) = inked(&render(&screen, "og_plus", "gray-4"));
    let middle = (columns.first().unwrap() + columns.last().unwrap()) / 2;
    assert!(
        middle.abs_diff(400) < 20,
        "the item's ink is centred on x={middle}"
    );
}

#[test]
fn a_column_that_does_not_grow_is_as_wide_as_its_content() {
    // Beside a column that grows, a column of one short title ends at the right edge.
    let screen = json!({ "content": stack("row", false, &[
        stack("column", true, &[title("Grows")]),
        stack("column", false, &[title("Right")]),
    ]) });
    let (_, columns) = inked(&render(&screen, "og_plus", "gray-4"));
    let first = *columns.first().unwrap();
    let last = *columns.last().unwrap();
    assert!(first < 40 && last > 700, "ink spans x={first}..{last}");
}

#[test]
fn a_missing_image_or_value_leaves_the_rest_of_the_screen() {
    let screen = json!({ "content": { "column": { "gap": "medium", "align": "start", "justify": "start", "grow": false, "children": [
        { "image": { "url": null, "fit": "contain", "grow": false } },
        { "lineChart": { "grow": true, "series": [ { "values": [1, null, "3.5", "unavailable", 2, 4] } ] } },
        { "barChart": { "grow": true, "series": [ { "values": [null, "2", 3] } ] } }
    ] } } });
    assert!(inked_rows(&render(&screen, "og_plus", "gray-4")) > 50);
}

#[test]
fn supports_what_it_can_write_and_has_tokens_for() {
    let catalog = Catalog::get();
    let drawable = |model: &str, palette: &str| {
        Renderer::supports(
            catalog.model(model).unwrap(),
            catalog.palette(palette).unwrap(),
        )
    };
    assert!(drawable("og_plus", "gray-4"));
    // These take WebP images.
    for model in ["openframe", "meta_portal", "tcl_nxtpaper_14"] {
        let palettes = &catalog.model(model).unwrap().palette_ids;
        assert!(
            palettes.iter().all(|palette| !drawable(model, palette)),
            "{model}"
        );
    }
}

#[test]
fn every_default_display_is_one_it_can_draw() {
    let catalog = Catalog::get();
    for (firmware, size) in [
        ("unknown", None),
        ("xiao_epaper_display", Some((800, 480))),
        ("openframe", None),
        ("tidbyt", Some((64, 32))),
    ] {
        let (model, palette) = catalog.default_display(firmware, size, Renderer::supports);
        assert!(
            Renderer::supports(model, palette),
            "{firmware}: {}",
            model.name
        );
    }
}

#[test]
fn a_progress_bar_fills_its_share_of_the_track() {
    // Emphasis 3 fills solid black on four grays; the track is 32 pixels tall at
    // the large size and spans the 780 pixels inside the screen's margins, a
    // one-pixel border around its inside.
    let bar =
        json!({ "progressBar": { "value": 25, "max": "100", "size": "large", "emphasis": 3 } });
    let bytes = render(&screen_of(&bar), "og_plus", "gray-4");
    let row = &pixels(&bytes)[10 + 16];
    let black = row.iter().filter(|pixel| **pixel == [0, 0, 0]).count();
    // A quarter of the inside, to two pixels: four times the black pixels is
    // within eight of the inside's width.
    let inside: usize = 780 - 2;
    assert!(
        (black * 4).abs_diff(inside) <= 8,
        "{black} black pixels across a track {inside} wide"
    );
}

#[test]
fn progress_dots_fill_their_first_value() {
    // Large dots are 20 pixels across, 7 apart, from the margin at 10: white in
    // the gaps between them and after the last.
    let dots =
        json!({ "progressDots": { "count": "5", "value": 2, "current": true, "size": "large" } });
    let bytes = render(&screen_of(&dots), "og_plus", "gray-4");
    let row = &pixels(&bytes)[10 + 10];
    let at = |dot: usize, offset: usize| row[10 + dot * 27 + offset];
    for dot in 0..5 {
        assert_eq!(at(dot, 23), [255, 255, 255], "the gap after dot {dot}");
    }
    assert!(
        row[10 + 5 * 27..]
            .iter()
            .all(|pixel| *pixel == [255, 255, 255])
    );
    assert_eq!((at(0, 10), at(1, 10)), ([0, 0, 0], [0, 0, 0]), "filled");
    for dot in 2..5 {
        assert_ne!(at(dot, 10), [0, 0, 0], "dot {dot} is not filled");
    }
}

#[test]
fn a_table_draws_a_rule_under_the_headings_and_between_rows() {
    // A small table: a 24-pixel heading row and 31-pixel body rows from the
    // margin at 10, each rule on its row's bottom pixel row, none under the last
    // row unless a row saying what was left out follows. Short text leaves the
    // right of the screen to the rules.
    let table = |more: Value| {
        json!({ "table": {
        "size": "small", "indexed": false, "grow": false, "more": more,
        "columns": [
            { "heading": "A", "align": "start", "clamp": 1 },
            { "heading": "B", "align": "start", "clamp": 1 }
        ],
        "rows": [["a", "1"], ["b", "2"], ["c", "3"]]
    } })
    };
    let rules = |bytes: &[u8]| {
        pixels(bytes)
            .iter()
            .enumerate()
            .filter(|(_, row)| row[700..780].iter().any(|pixel| *pixel != [255, 255, 255]))
            .map(|(y, _)| y)
            .collect::<Vec<_>>()
    };
    let bytes = render(&screen_of(&table(Value::Null)), "og_png", "bw");
    assert_eq!(rules(&bytes), [33, 64, 95]);
    let bytes = render(&screen_of(&table(json!("and 1 more"))), "og_png", "bw");
    assert_eq!(rules(&bytes), [33, 64, 95, 126]);
}

#[test]
fn an_inverse_surface_draws_light_content_on_a_dark_one() {
    let cell = json!({ "column": { "inverse": true, "padding": "small", "rounded": "none",
        "gap": "medium", "align": "start", "justify": "start", "grow": false,
        "children": [{ "title": { "text": "Hi", "size": "base" } }] } });
    let bytes = render(&screen_of(&cell), "og_plus", "gray-4");
    let pixels = pixels(&bytes);
    assert_eq!(pixels[11][11], [0, 0, 0], "the surface is dark");
    let inside: Vec<[u8; 3]> = pixels[10..40]
        .iter()
        .flat_map(|row| row[10..60].iter().copied())
        .collect();
    assert!(inside.contains(&[255, 255, 255]), "its text is light");
}

#[test]
fn small_light_text_on_an_inverse_surface_keeps_its_strokes_in_black_and_white() {
    // The same label, in a face whose strokes land between pixels, dark on white
    // and light on an inverse surface: the light one keeps as many pixels as the
    // dark one, since a stroke is a stroke whichever its color.
    let label = json!({ "label": { "text": "Today IIIIII", "size": "large", "style": "plain" } });
    let plain = pixels(&render(&screen_of(&label), "og_png", "bw"));
    let dark = plain
        .iter()
        .flatten()
        .filter(|pixel| **pixel == [0, 0, 0])
        .count();
    let cell = json!({ "column": { "inverse": true, "padding": "small", "rounded": "none",
        "gap": "medium", "align": "start", "justify": "start", "grow": false, "children": [label] } });
    let inverse = pixels(&render(&screen_of(&cell), "og_png", "bw"));
    // The surface spans the screen's width; its right end holds no text.
    let surface = (10..inverse.len())
        .take_while(|&y| inverse[y][785] == [0, 0, 0])
        .count();
    assert!(surface > 20, "a surface {surface} rows tall");
    let light = inverse[10..10 + surface]
        .iter()
        .flat_map(|row| row[10..790].iter())
        .filter(|pixel| **pixel == [255, 255, 255])
        .count();
    assert!(
        light * 100 >= dark * 95,
        "{light} light pixels for {dark} dark ones"
    );
}

#[test]
fn rich_text_sets_strong_runs_heavier_and_wraps_at_its_width() {
    let rich = |text: &str| json!({ "richText": { "size": "base", "align": "start", "gap": "medium", "text": text } });
    let ink = |bytes: &[u8]| {
        pixels(bytes)
            .iter()
            .flatten()
            .filter(|pixel| **pixel == [0, 0, 0])
            .count()
    };
    let regular = ink(&render(&screen_of(&rich("WWWWWW")), "og_png", "bw"));
    let strong = ink(&render(&screen_of(&rich("**WWWWWW**")), "og_png", "bw"));
    assert!(
        regular > 0 && strong > regular,
        "{strong} strong against {regular} regular"
    );

    let bytes = render(&screen_of(&rich(&"word ".repeat(80))), "og_png", "bw");
    let (rows, columns) = inked(&bytes);
    assert!(rows.len() > 16, "it wraps onto several lines");
    assert!(
        columns.last().unwrap() - columns.first().unwrap() < 640,
        "at most 640 pixels wide"
    );
}
