//! OxiTide TIDAL Connect probe (spike for issue #11).
//!
//! Plays three tracks on a TIDAL Connect device using OxiTide's saved login,
//! then tries pause, play and skip, ends the session and deletes the cloud
//! queue it created. The log it writes leaves out the login token, the
//! account id and the country, so it can be pasted into a GitHub issue.

mod bench;
mod cast;
mod discover;
mod interrupt;
mod log;
mod scrub;
mod tidal;
mod tls;

use std::path::PathBuf;

use anyhow::{bail, Result};

use crate::cast::{CastOptions, Findings};
use crate::log::Log;

const HELP: &str = "\
oxitide-connect-probe: tests whether a TIDAL Connect device (e.g. a WiiM) accepts OxiTide.

Usage:
  oxitide-connect-probe                 same as `cast` (finds the device on its own)
  oxitide-connect-probe cast [options]  play 3 tracks, pause, play, skip, stop
  oxitide-connect-probe discover        only list TIDAL Connect devices on the network
  oxitide-connect-probe tls-check HOST:PORT
                                        only check the device's TLS certificate (sends nothing)

Options for cast:
  --device NAME|HOST:PORT   pick a device by (part of) its name, or connect directly
  --tracks ID,ID,ID         TIDAL track ids to queue (default: three search results)
  --quality Q               LOW | HIGH | LOSSLESS | HI_RES_LOSSLESS (default LOSSLESS)
  --keyring-service NAME    OxiTide's keyring service (default: oxitide)
  --seconds N               longest wait for devices (default 20; ends 3 s after the first)
  --log FILE                log file (default: connect-probe-<time>.log in the current folder)

Requirements: OxiTide installed and signed in on this PC, and this PC on the same
network as the device. The probe only reads OxiTide's login; it never changes it.
Ctrl+C or closing the window still ends the session on the device and cleans up.
";

struct Args {
    command: String,
    positional: Vec<String>,
    device: Option<String>,
    tracks: Vec<u64>,
    quality: String,
    keyring_service: String,
    seconds: u64,
    log: Option<PathBuf>,
    port: u16,
    service_type: Option<String>,
    interactive: bool,
}

fn parse_args() -> Result<Args> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let interactive = raw.is_empty();
    let mut args = Args {
        command: "cast".into(),
        positional: Vec::new(),
        device: None,
        tracks: Vec::new(),
        quality: "LOSSLESS".into(),
        keyring_service: "oxitide".into(),
        seconds: 20,
        log: None,
        port: 9000,
        service_type: None,
        interactive,
    };
    let mut it = raw.into_iter();
    let mut first = true;
    while let Some(arg) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .ok_or_else(|| anyhow::anyhow!("{name} needs a value"))
        };
        match arg.as_str() {
            "-h" | "--help" | "help" => {
                args.command = "help".into();
            }
            "--device" => args.device = Some(value("--device")?),
            "--tracks" => {
                args.tracks = value("--tracks")?
                    .split(',')
                    .map(|s| s.trim().parse::<u64>())
                    .collect::<std::result::Result<_, _>>()
                    .map_err(|_| anyhow::anyhow!("--tracks takes numeric ids, e.g. 1,2,3"))?;
            }
            "--quality" => args.quality = value("--quality")?,
            "--keyring-service" => args.keyring_service = value("--keyring-service")?,
            "--seconds" => args.seconds = value("--seconds")?.parse()?,
            "--log" => args.log = Some(PathBuf::from(value("--log")?)),
            "--port" => args.port = value("--port")?.parse()?,
            "--service-type" => args.service_type = Some(value("--service-type")?),
            other if other.starts_with("--") => bail!("unknown option {other}\n\n{HELP}"),
            other if first => args.command = other.to_string(),
            other => args.positional.push(other.to_string()),
        }
        first = false;
    }
    Ok(args)
}

fn default_log_path() -> PathBuf {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    PathBuf::from(format!("connect-probe-{secs}.log"))
}

#[tokio::main]
async fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if args.command == "help" {
        print!("{HELP}");
        return;
    }
    if args.command != "bench-host" {
        interrupt::listen();
    }
    let log = Log::new(Some(args.log.clone().unwrap_or_else(default_log_path)));
    log.plain(format!(
        "oxitide-connect-probe {} ({}, {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    let code = match run(&args, &log).await {
        Ok(()) => 0,
        Err(e) => {
            log.line(format!("ERROR: {e:#}"));
            1
        }
    };
    if let Some(path) = log.path() {
        let shown = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        let shown = shown.display().to_string();
        let shown = shown.strip_prefix(r"\\?\").unwrap_or(&shown).to_string();
        println!("\nLog saved to: {shown}");
    }
    if args.interactive {
        println!("Press Enter to close.");
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
    }
    std::process::exit(code);
}

async fn run(args: &Args, log: &Log) -> Result<()> {
    match args.command.as_str() {
        "discover" => {
            match &args.service_type {
                // Developer check that multicast from other hosts reaches us.
                Some(ty) => discover::discover_type(log, ty, args.seconds, false).await?,
                None => discover::discover(log, args.seconds).await?,
            };
            Ok(())
        }
        "tls-check" => {
            let Some(target) = args.positional.first() else {
                bail!("tls-check needs HOST:PORT");
            };
            tls_check(log, target).await
        }
        "bench-host" => bench::run(log, args.port).await,
        "mdns-selftest" => discover::self_test(log).await,
        "cast" => {
            let opts = CastOptions {
                device: args.device.clone(),
                tracks: args.tracks.clone(),
                keyring_service: args.keyring_service.clone(),
                quality: args.quality.clone(),
                discover_seconds: args.seconds,
            };
            let mut findings = Findings::default();
            let result = cast::run(log, &opts, &mut findings).await;
            summary(log, &findings);
            result
        }
        other => bail!("unknown command {other}\n\n{HELP}"),
    }
}

/// TLS handshake only: is the device's certificate issued by TIDAL's CA?
async fn tls_check(log: &Log, target: &str) -> Result<()> {
    use tokio_tungstenite::Connector;
    let (config, verifier) = tls::client_config()?;
    let url = format!("wss://{target}");
    log.line(format!("TLS check against {target} (no data is sent)..."));
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio_tungstenite::connect_async_tls_with_config(
            url.as_str(),
            None,
            false,
            Some(Connector::Rustls(config)),
        ),
    )
    .await;
    let presented = verifier.presented();
    log.line(format!("{} certificate(s) presented.", presented.len()));
    match (result, verifier.failure()) {
        (_, Some(failure)) => {
            log.line(failure);
            for cert in presented {
                log.plain(tls::to_pem(&cert));
            }
            bail!("the certificate does not chain to TIDAL's Connect CA")
        }
        (Err(_), None) => bail!("timed out"),
        (Ok(Ok((mut ws, _))), None) => {
            log.line("TLS OK and WebSocket upgrade accepted.");
            let _ = ws.close(None).await;
            Ok(())
        }
        (Ok(Err(e)), None) if !presented.is_empty() => {
            log.line(format!(
                "TLS OK (certificate accepted); WebSocket upgrade then failed: {e}"
            ));
            Ok(())
        }
        (Ok(Err(e)), None) => bail!("connection failed: {e}"),
    }
}

fn yes(v: bool) -> &'static str {
    if v {
        "yes"
    } else {
        "no"
    }
}

fn summary(log: &Log, f: &Findings) {
    log.plain("\n===== Summary (paste everything in this log into the issue) =====");
    log.plain(format!(
        "Device:                  {}",
        f.device.as_deref().unwrap_or("-")
    ));
    log.plain(format!("Cloud queue created:     {}", yes(f.queue_created)));
    log.plain(format!(
        "TLS (TIDAL CA):          {}",
        match f.tls_ok {
            Some(true) => "ok",
            Some(false) => "REJECTED",
            None => "not reached",
        }
    ));
    log.plain(format!(
        "Session started:         {}",
        yes(f.session_started)
    ));
    log.plain(format!(
        "Device reported PLAYING: {}",
        yes(f.saw_state("PLAYING"))
    ));
    log.plain(format!(
        "Started on its own:      {}",
        match f.autoplay {
            Some(true) => "yes",
            Some(false) => "no (needed an explicit play)",
            None => "-",
        }
    ));
    for (command, label) in [
        ("pause", "pause -> PAUSED:         "),
        ("play", "play -> PLAYING:         "),
        ("next", "next -> track 2:         "),
    ] {
        let result = f
            .transport
            .iter()
            .find(|(c, _)| *c == command)
            .map(|(_, ok)| yes(*ok))
            .unwrap_or("not reached");
        log.plain(format!("{label}{result}"));
    }
    log.plain(format!(
        "Server errors:           {}",
        if f.server_errors.is_empty() {
            "none".to_string()
        } else {
            f.server_errors.join(" | ")
        }
    ));
    if !f.session_errors.is_empty() {
        log.plain(format!(
            "Session errors:          {}",
            f.session_errors.join(" | ")
        ));
    }
    if !f.request_results.is_empty() {
        log.plain(format!(
            "Request results:         {}",
            f.request_results.join(" | ")
        ));
    }
    log.plain(format!(
        "Connection:              {}",
        match (f.closed_by_device, f.end_sent) {
            (false, _) => "closed by the probe",
            (true, true) => "closed by the device after endSession (expected)",
            (true, false) => "CLOSED BY THE DEVICE EARLY",
        }
    ));
    log.plain(format!("Cloud queue deleted:     {}", yes(f.queue_deleted)));
    log.plain(
        "\nPlease also tell us: did you hear the music, what did the WiiM Home app show,\n\
         and does OxiTide still play normally when you open it now?\n\
         The log contains no login, account id or country, but it does show the device's\n\
         name and its address on your home network; edit those out before posting if you like.",
    );
    if f.interrupted {
        log.plain("(The run was interrupted before it finished.)");
    }
}
