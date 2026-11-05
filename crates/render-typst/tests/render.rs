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
