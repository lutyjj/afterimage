//! Reads TRMNL's API document on standard input and writes the device contract
//! excerpt that the conformance test compares the model against.
//!
//! Usage: `trmnl-excerpt --source URL --retrieved YYYY-MM-DD < openapi.json > device-api.json`

use std::io::Read;
use std::process::ExitCode;

use afterimage_conformance::{DEVICE_OPERATIONS, Excerpt, extract};

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
    let usage = "usage: trmnl-excerpt --source URL --retrieved YYYY-MM-DD < openapi.json";
    let (Some("--source"), Some(source), Some("--retrieved"), Some(retrieved), None) = (
        args.next().as_deref(),
        args.next(),
        args.next().as_deref(),
        args.next(),
        args.next(),
    ) else {
        return Err(usage.into());
    };
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| format!("reading standard input: {error}"))?;
    let document = serde_json::from_str(&input).map_err(|error| format!("not JSON: {error}"))?;
    let excerpt = Excerpt {
        source,
        retrieved,
        contract: extract(&document, &DEVICE_OPERATIONS)?,
    };
    let json = serde_json::to_string_pretty(&excerpt).map_err(|error| error.to_string())?;
    println!("{json}");
    Ok(())
}
