//! Reads TRMNL's API document on standard input and writes the device contract
//! excerpt that the conformance test compares the model against.
//!
//! Usage: `trmnl-excerpt --retrieved YYYY-MM-DD < openapi.json > device-api.json`

use std::io::Read;
use std::process::ExitCode;

use afterimage_conformance::{DEVICE_OPERATIONS, Excerpt, TRMNL_DOCUMENT, extract};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("trmnl-excerpt: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (Some("--retrieved"), Some(retrieved), None) =
        (args.next().as_deref(), args.next(), args.next())
    else {
        return Err("usage: trmnl-excerpt --retrieved YYYY-MM-DD < openapi.json".into());
    };
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| format!("reading standard input: {error}"))?;
    let document = serde_json::from_str(&input).map_err(|error| format!("not JSON: {error}"))?;
    let excerpt = Excerpt {
        source: TRMNL_DOCUMENT.to_owned(),
        retrieved,
        contract: extract(&document, &DEVICE_OPERATIONS)?,
    };
    let json = serde_json::to_string_pretty(&excerpt).map_err(|error| error.to_string())?;
    println!("{json}");
    Ok(())
}
