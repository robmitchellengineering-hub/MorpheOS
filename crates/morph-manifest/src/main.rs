//! `morph-manifest`: validate, render and plan MorpheOS Manifests.

use std::process::ExitCode;

use morph_manifest::{Error, Severity, plan_values, resolve_value, validate};

const USAGE: &str = "\
usage:
  morph-manifest validate <machine.toml>       check structure and invariants
  morph-manifest render   <machine.toml>       print the merged manifest as JSON
  morph-manifest plan     <old.toml> <new.toml> show changes, risk, and whether it may auto-apply
  morph-manifest schema                        print the JSON Schema";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["validate", file] => cmd_validate(file),
        ["render", file] => cmd_render(file),
        ["plan", old, new] => cmd_plan(old, new),
        ["schema"] => {
            println!("{:#}", morph_manifest::json_schema());
            Ok(true)
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn load(file: &str) -> Result<serde_json::Value, Error> {
    let source = std::fs::read_to_string(file).map_err(|e| Error::Parse {
        origin: file.to_string(),
        message: e.to_string(),
    })?;
    resolve_value(&source, file)
}

fn cmd_validate(file: &str) -> Result<bool, Error> {
    let manifest = morph_manifest::layer::from_value(load(file)?)?;
    let report = validate(&manifest);
    for f in &report.findings {
        let tag = match f.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        println!("{tag} [{}] {}: {}", f.code, f.path, f.message);
    }
    if report.is_ok() {
        println!(
            "ok: {} ({})",
            manifest.machine.name,
            manifest.machine.profile.as_str()
        );
    }
    Ok(report.is_ok())
}

fn cmd_render(file: &str) -> Result<bool, Error> {
    let value = load(file)?;
    morph_manifest::layer::from_value(value.clone())?;
    println!("{value:#}");
    Ok(true)
}

fn cmd_plan(old: &str, new: &str) -> Result<bool, Error> {
    let plan = plan_values(&load(old)?, &load(new)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&plan).expect("plan serializes")
    );
    Ok(plan.report.is_ok())
}
