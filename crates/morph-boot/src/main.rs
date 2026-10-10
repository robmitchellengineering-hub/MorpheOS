//! `morph-boot`: inspect and drive the A/B boot slots.

use std::process::{Command, ExitCode};

use morph_boot::{BootControl, Decision, Slot, pi::Pi, sdboot::SdBoot};

const USAGE: &str = "\
usage: morph-boot [backend options] <command>

commands:
  status                     print booted slot, default slot and phase (JSON)
  target                     print the slot an update may be written to
  trial <a|b> [--reboot]     arm a one-shot trial boot of a freshly written slot
  confirm --healthy|--unhealthy [--reboot]
                             after the health check: commit a trial, or fall back

backend options (default: detected):
  --backend pi|sdboot
  --autoboot PATH            pi: autoboot.txt        (default /bootfs/autoboot.txt)
  --dt PATH                  pi: bootloader DT node  (default /proc/device-tree/chosen/bootloader)
  --esp PATH                 sdboot: EFI system partition (default /efi)
  --cmdline PATH             sdboot: kernel command line  (default /proc/cmdline)";

struct Opts {
    backend: Option<String>,
    autoboot: String,
    dt: String,
    esp: String,
    cmdline: String,
    rest: Vec<String>,
}

fn parse(args: Vec<String>) -> Option<Opts> {
    let mut o = Opts {
        backend: None,
        autoboot: "/bootfs/autoboot.txt".into(),
        dt: "/proc/device-tree/chosen/bootloader".into(),
        esp: "/efi".into(),
        cmdline: "/proc/cmdline".into(),
        rest: Vec::new(),
    };
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--backend" => o.backend = Some(it.next()?),
            "--autoboot" => o.autoboot = it.next()?,
            "--dt" => o.dt = it.next()?,
            "--esp" => o.esp = it.next()?,
            "--cmdline" => o.cmdline = it.next()?,
            _ => o.rest.push(a),
        }
    }
    Some(o)
}

fn backend(o: &Opts) -> Box<dyn BootControl> {
    let kind = o.backend.clone().unwrap_or_else(|| {
        if std::path::Path::new(&o.dt).join("partition").exists() {
            "pi".into()
        } else {
            "sdboot".into()
        }
    });
    if kind == "pi" {
        Box::new(Pi::new(&o.autoboot, &o.dt))
    } else {
        let mut s = SdBoot::new(&o.esp);
        s.cmdline = o.cmdline.clone().into();
        Box::new(s)
    }
}

fn run(cmd: &[String]) -> Result<(), String> {
    let status = Command::new(&cmd[0])
        .args(&cmd[1..])
        .status()
        .map_err(|e| format!("{}: {e}", cmd[0]))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{} exited with {status}", cmd.join(" ")))
}

/// Render a command so it can be pasted into a shell as-is.
fn shell_words(cmd: &[String]) -> String {
    cmd.iter()
        .map(|w| {
            if w.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./=".contains(c))
            {
                w.clone()
            } else {
                format!("'{}'", w.replace('\'', r"'\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() -> ExitCode {
    let Some(o) = parse(std::env::args().skip(1).collect()) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let mut bc = backend(&o);
    let rest: Vec<&str> = o.rest.iter().map(String::as_str).collect();
    let result: Result<(), String> = match rest.as_slice() {
        ["status"] => morph_boot::status(bc.as_ref())
            .map(|s| println!("{}", serde_json::to_string(&s).unwrap()))
            .map_err(|e| e.to_string()),
        ["target"] => morph_boot::update_target(bc.as_ref())
            .map(|s| println!("{s}"))
            .map_err(|e| e.to_string()),
        ["trial", slot, flags @ ..] => match Slot::parse(slot) {
            None => Err(format!("unknown slot '{slot}'")),
            Some(slot) => morph_boot::start_trial(bc.as_mut(), slot)
                .map_err(|e| e.to_string())
                .and_then(|reboot| {
                    println!(
                        "trial armed for slot {slot}; reboot with: {}",
                        shell_words(&reboot)
                    );
                    if flags.contains(&"--reboot") {
                        run(&reboot)
                    } else {
                        Ok(())
                    }
                }),
        },
        ["confirm", health, flags @ ..] if *health == "--healthy" || *health == "--unhealthy" => {
            morph_boot::confirm(bc.as_mut(), *health == "--healthy")
                .map_err(|e| e.to_string())
                .and_then(|d| {
                    println!("{}", serde_json::to_string(&d).unwrap());
                    match d {
                        Decision::Fallback { reboot, .. } if flags.contains(&"--reboot") => {
                            run(&reboot)
                        }
                        _ => Ok(()),
                    }
                })
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("morph-boot: {e}");
            ExitCode::FAILURE
        }
    }
}
