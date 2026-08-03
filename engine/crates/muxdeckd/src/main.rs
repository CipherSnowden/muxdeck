//! `muxdeckd` — the MuxDeck daemon.
//!
//! This binary owns nothing but startup: command-line parsing, config directory resolution,
//! tracing setup, then handing control to `muxdeck-engine`. Real work belongs in the library so
//! it stays testable. CLI surface: `docs/ENGINE.md` §7.

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use muxdeck_engine::admin_client::AdminClient;
use muxdeck_engine::config::Paths;
use muxdeck_engine::discovery::Advertisement;
use muxdeck_engine::muxdeck_core::KnownOp;
use muxdeck_engine::{server, service, Engine};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "muxdeckd",
    version,
    about = "The MuxDeck daemon: receives button presses over the LAN and injects input."
)]
struct Cli {
    /// Override the listen port.
    #[arg(long)]
    port: Option<u16>,

    /// Override the configuration directory.
    #[arg(long)]
    config_dir: Option<PathBuf>,

    /// trace | debug | info | warn | error
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Log to stdout instead of the log file.
    #[arg(long)]
    foreground: bool,

    /// Print the TLS certificate fingerprint and exit.
    #[arg(long)]
    print_fingerprint: bool,

    /// Print the resolved configuration directory and exit.
    #[arg(long)]
    print_config_dir: bool,

    /// Regenerate the host key and certificate. Unpairs every device.
    #[arg(long)]
    reset_identity: bool,

    /// Confirm a destructive operation without prompting.
    #[arg(long)]
    yes: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Manage device pairing without the desktop panel.
    #[command(subcommand)]
    Pair(PairCommand),

    /// Register the daemon to start automatically.
    #[command(subcommand)]
    Service(ServiceCommand),
}

#[derive(Subcommand)]
enum PairCommand {
    /// Open a pairing window and print the code and QR payload.
    Begin {
        /// Window length in seconds, 30 to 300.
        #[arg(long)]
        ttl: Option<u32>,
    },
    /// List paired devices.
    List,
    /// Remove a paired device.
    Revoke { device_id: String },
}

#[derive(Subcommand)]
enum ServiceCommand {
    /// Register the daemon to start at logon for the current user.
    Install,
    /// Remove the auto-start registration.
    Uninstall,
    /// Report whether auto-start is registered and whether it is running.
    Status,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // rustls 0.23 refuses to do anything until a crypto provider is installed, and the panic it
    // raises otherwise reads like a configuration error rather than a missing call.
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("a rustls crypto provider was already installed"))?;

    init_tracing(&cli.log_level, cli.foreground)?;

    let paths = Paths::resolve(cli.config_dir.clone()).context("resolving the config directory")?;

    if cli.print_config_dir {
        println!("{}", paths.root().display());
        return Ok(());
    }

    if cli.reset_identity {
        return reset_identity(&paths, cli.yes);
    }

    // Cloned rather than moved because `service` needs the config directory too, and loading the
    // engine first is deliberate: `service install` is often the very first command a fresh
    // install runs, and it should leave the identity and certificate generated behind it.
    let engine = Engine::load(paths.clone()).context("loading engine state")?;

    if cli.print_fingerprint {
        println!("{}", engine.identity.fingerprint());
        return Ok(());
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("starting the async runtime")?;

    match cli.command {
        Some(Command::Pair(command)) => runtime.block_on(run_pair(&engine, command)),
        Some(Command::Service(command)) => run_service(command, paths.root()),
        None => runtime.block_on(run_daemon(engine, cli.port)),
    }
}

/// Runs the engine until Ctrl-C.
async fn run_daemon(engine: std::sync::Arc<Engine>, port_override: Option<u16>) -> Result<()> {
    let port = port_override.unwrap_or_else(|| engine.settings().port);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    let running = server::serve(engine.clone(), addr)
        .await
        .context("starting the websocket server")?;

    // Logged at INFO on purpose so the user can compare it against what their phone shows
    // during pairing. The admin token and every key are never logged at any level.
    info!(
        host_id = engine.identity.host_id(),
        fingerprint = engine.identity.fingerprint(),
        address = %running.addr,
        "muxdeckd is listening"
    );

    let advertisement = Advertisement::start(
        &engine.host_name(),
        engine.identity.host_id(),
        engine.identity.fingerprint(),
        running.addr.port(),
    )
    .context("advertising over mDNS")?;

    tokio::signal::ctrl_c()
        .await
        .context("waiting for Ctrl-C")?;
    info!("shutting down");

    advertisement.stop();
    running.shutdown();
    Ok(())
}

/// The `pair` subcommands, over an ordinary loopback admin connection.
async fn run_pair(engine: &Engine, command: PairCommand) -> Result<()> {
    let mut client = AdminClient::connect(
        engine.settings().port,
        engine.identity.fingerprint(),
        engine.identity.admin_token(),
    )
    .await
    .context("connecting to the running engine")?;

    match command {
        PairCommand::Begin { ttl } => {
            let response = client
                .request(
                    KnownOp::PairBegin,
                    &serde_json::json!({ "ttl_seconds": ttl }),
                )
                .await
                .context("opening a pairing window")?;

            let code = response["code"].as_str().unwrap_or("??????");
            let qr = response["qr_payload"].as_str().unwrap_or("");
            println!("Pairing code: {code}");
            println!("QR payload:   {qr}");
            println!();
            println!(
                "Scan the QR payload with the MuxDeck app, or enter the address and code by hand."
            );
        }
        PairCommand::List => {
            let response = client
                .request(KnownOp::PairListDevices, &serde_json::json!({}))
                .await
                .context("listing devices")?;

            let devices = response["devices"].as_array().cloned().unwrap_or_default();
            if devices.is_empty() {
                println!("No devices paired. Run `muxdeckd pair begin` to add one.");
            }
            for device in devices {
                println!(
                    "{}  {:<24} {:<8} {}",
                    device["device_id"].as_str().unwrap_or(""),
                    device["name"].as_str().unwrap_or(""),
                    device["platform"].as_str().unwrap_or(""),
                    if device["connected"].as_bool().unwrap_or(false) {
                        "connected"
                    } else {
                        ""
                    }
                );
            }
        }
        PairCommand::Revoke { device_id } => {
            client
                .request(
                    KnownOp::PairRevoke,
                    &serde_json::json!({ "device_id": device_id }),
                )
                .await
                .with_context(|| format!("revoking {device_id}"))?;
            println!("Revoked {device_id}.");
        }
    }
    Ok(())
}

/// Auto-start registration, per platform. `muxdeck_engine::service` explains why Windows gets a
/// Scheduled Task and not a Windows Service.
///
/// `install` prints its follow-up notes rather than acting on them: on Linux the `/dev/uinput`
/// permission fix needs root, and an installer that escalates on its own is one the user cannot
/// audit. Blank lines between notes because each is a paragraph or a block of shell commands.
fn run_service(command: ServiceCommand, config_dir: &std::path::Path) -> Result<()> {
    match command {
        ServiceCommand::Install => {
            let installed = service::install(config_dir).context("registering auto-start")?;
            println!("Auto-start registered: {}", installed.location);
            for note in &installed.notes {
                println!();
                println!("{note}");
            }
        }
        ServiceCommand::Uninstall => {
            service::uninstall().context("removing the auto-start registration")?;
            println!("Auto-start removed. The daemon keeps running until you stop it.");
        }
        ServiceCommand::Status => {
            let status = service::status().context("reading the auto-start registration")?;
            println!("{status}");
        }
    }
    Ok(())
}

/// Regenerates the host identity. Destructive: every paired device stops working.
fn reset_identity(paths: &Paths, confirmed: bool) -> Result<()> {
    if !confirmed {
        bail!(
            "--reset-identity regenerates the host key and certificate, which unpairs every \
             device. Re-run with --yes to confirm."
        );
    }

    for path in [
        paths.identity_key(),
        paths.tls_cert(),
        paths.tls_key(),
        paths.admin_token(),
        paths.devices(),
    ] {
        match std::fs::remove_file(&path) {
            Ok(()) => info!(path = %path.display(), "removed"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("removing {}", path.display())),
        }
    }

    println!("Identity reset. Every device must be paired again.");
    Ok(())
}

fn init_tracing(level: &str, foreground: bool) -> Result<()> {
    let filter =
        EnvFilter::try_new(level).with_context(|| format!("'{level}' is not a valid log level"))?;

    // A file appender would need a guard held for the process lifetime; until the daemon has a
    // long-lived owner for it (M5, alongside service installation) both paths go to stdout and
    // the flag only documents the intent.
    let _ = foreground;
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}
