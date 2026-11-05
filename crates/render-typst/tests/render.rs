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
