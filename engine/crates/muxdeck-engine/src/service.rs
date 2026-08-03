//! Auto-start registration for the current user. `docs/ENGINE.md` §7, `docs/SERVER.md` §5.
//!
//! Three platform mechanisms, one rule behind all of them: the daemon has to run **inside the
//! user's desktop session**. Injecting input is only meaningful there, so every arm below
//! registers a per-user, at-logon mechanism and none of them asks for root.
//!
//! That rule is what rules out a Windows Service. A service runs in session 0, which has no
//! desktop to inject into: the deck would pair, connect, report every press as delivered, and
//! nothing would happen on screen — a failure with no error anywhere to explain it. A Scheduled
//! Task with an at-logon trigger runs in the user's own session and needs no elevation, so that
//! is what `install` creates. Do not "upgrade" it to a service.
//!
//! Registration is deliberately the only privileged thing the engine does, and on Linux it is
//! not even that: `/dev/uinput` access needs root, so `install` writes the udev rule out and
//! prints the commands rather than running them behind the user's back.

use std::path::Path;

use crate::error::Result;

/// Scheduled Task name on Windows. Also what `schtasks /Query /TN` is asked about.
pub const WINDOWS_TASK_NAME: &str = "MuxDeck";

/// File name the Windows task definition is written under, inside the config directory.
///
/// It is kept rather than written to a temp file and deleted: `schtasks` copies it into the Task
/// Scheduler store, so the copy left behind is purely a record of what was registered — which is
/// the first thing worth looking at when auto-start misbehaves.
pub const WINDOWS_TASK_FILE: &str = "scheduled-task.xml";

/// launchd label on macOS. Doubles as the plist file stem, per launchd convention.
pub const LAUNCHD_LABEL: &str = "in.redoimagined.muxdeck";

/// systemd user unit name on Linux.
pub const SYSTEMD_UNIT_NAME: &str = "muxdeck.service";

/// File name the Linux udev rule is written under, in the config directory and again under
/// `/etc/udev/rules.d/`. The `99-` prefix keeps it last so it overrides distro defaults.
pub const UDEV_RULE_FILE: &str = "99-muxdeck-uinput.rules";

/// The udev rule that makes `/dev/uinput` writable by the `input` group. `docs/ENGINE.md` §4.3.
///
/// Installing this needs root, so `install` only writes it into the config directory and tells
/// the user the two commands to run. Doing it silently with a `sudo` prompt from a background
/// installer step is exactly the behaviour a user should distrust.
pub const UDEV_RULE: &str = concat!(
    "# MuxDeck: let members of the `input` group open /dev/uinput for writing.\n",
    "KERNEL==\"uinput\", GROUP=\"input\", MODE=\"0660\"\n"
);

/// Whether auto-start is registered, and if so whether it is currently up.
///
/// Tri-state rather than a bool because the panel offers a different button for each: *Install*
/// for [`ServiceStatus::NotInstalled`], *Start* for [`ServiceStatus::InstalledStopped`], and
/// nothing at all for [`ServiceStatus::Running`]. Collapsing the middle case into "not
/// installed" would make the panel re-register a task that already exists every time the daemon
/// happened to be down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    /// No task, agent or unit exists for this user.
    NotInstalled,
    /// Registered to start at logon, but not running right now.
    InstalledStopped,
    /// Registered and currently running.
    Running,
}

impl ServiceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ServiceStatus::NotInstalled => "not installed",
            ServiceStatus::InstalledStopped => "installed, not running",
            ServiceStatus::Running => "installed and running",
        }
    }
}

impl std::fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What `install` did, and what it deliberately left for the user to do.
#[derive(Debug, Clone)]
pub struct Installation {
    /// Where the registration now lives, so it can be inspected or undone by hand.
    pub location: String,
    /// Follow-up steps the tool did not take — anything needing root — plus any warning about
    /// where the daemon binary was found. Printed verbatim by `muxdeckd service install`.
    pub notes: Vec<String>,
}

/// Registers the daemon to start at logon for the current user.
///
/// `config_dir` receives whatever supporting file the platform needs the user to be able to see:
/// the task definition on Windows, the udev rule on Linux. macOS needs neither — its plist is
/// already a plain file at a well-known path.
///
/// Idempotent: re-running replaces an existing registration rather than failing, so the panel
/// can offer Install without first having to check.
pub fn install(config_dir: &Path) -> Result<Installation> {
    platform::install(config_dir)
}

/// Removes the registration. Succeeds when there was nothing to remove.
pub fn uninstall() -> Result<()> {
    platform::uninstall()
}

pub fn status() -> Result<ServiceStatus> {
    platform::status()
}

// ---------------------------------------------------------------------------
// Content generation and output parsing.
//
// These are compiled on every target, not just the one that uses them, so all three are asserted
// by the test suite on all three CI runners instead of each being covered only by its own. A
// malformed plist or unit file fails at the next logon, on a machine the author is not sitting
// at — the worst possible place to find out — and none of it can be caught by a type.
// ---------------------------------------------------------------------------

/// The Scheduled Task definition handed to `schtasks /Create /XML`.
///
/// The obvious spelling — `schtasks /Create /SC ONLOGON /TN MuxDeck /TR <exe> /F` — is rejected
/// with "Access is denied." for a non-elevated caller, and so is every variation with `/RU`.
/// Verified on Windows 11 26200. Two separate reasons, both fixed here:
///
/// * `/SC ONLOGON` emits a `LogonTrigger` with **no** `UserId`, which means "at logon of any
///   user" — a machine-wide change, so it needs administrator rights. Naming the current user in
///   the trigger makes it a per-user registration, which does not.
/// * `/RU <user>` without `/RP` selects the `S4U` logon type, which requires `SeTcbPrivilege`.
///   `InteractiveToken` needs no privilege and no stored password, and is the correct choice
///   regardless: the daemon must run in the user's interactive session or it cannot inject.
///
/// `RunLevel` stays `LeastPrivilege`. Elevating would only buy the ability to inject into other
/// elevated windows (`docs/ENGINE.md` §4.1), at the cost of demanding admin on every install.
///
/// Three of the settings below override defaults that would each break the daemon silently:
///
/// * `ExecutionTimeLimit` defaults to `PT72H`, after which Task Scheduler terminates the task.
///   A deck that stops responding after three days of uptime is a horrible bug to chase.
/// * `DisallowStartIfOnBatteries` and `StopIfGoingOnBatteries` both default to true, so on a
///   laptop the deck would die the moment the charger came out.
/// * `MultipleInstancesPolicy` defaults to `IgnoreNew` already, but is stated explicitly because
///   fast user switching re-fires the logon trigger, and a second daemon would fight the first
///   for the listen port.
pub fn windows_task_xml(exe: &Path, user: &str) -> String {
    let exe = xml_escape(&exe.display().to_string());
    let user = xml_escape(user);
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Starts the MuxDeck daemon in this user's desktop session at logon.</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>3</Count>
    </RestartOnFailure>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
    </Exec>
  </Actions>
</Task>
"#
    )
}

/// Encodes `text` as UTF-16LE with a byte-order mark.
///
/// `schtasks /Create /XML` hands the file to MSXML, which detects the encoding from the bytes and
/// then compares it against the declaration. A UTF-8 file saying `encoding="UTF-8"` fails with
/// `unable to switch the encoding` at column 40 — the declaration itself. UTF-16LE with a BOM,
/// declared as UTF-16, is what Task Scheduler's own export produces and what it reliably reads.
pub fn utf16le_with_bom(text: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(2 + text.len() * 2);
    bytes.extend_from_slice(&[0xFF, 0xFE]);
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

/// The LaunchAgent plist written to `~/Library/LaunchAgents/`.
///
/// `KeepAlive` is a dict rather than `true` on purpose. Plain `true` resurrects the daemon
/// within seconds of any exit, including the panel's own "Stop engine" menu item
/// (`docs/SERVER.md` §7) — the user would click Stop and watch it come straight back.
/// `SuccessfulExit: false` restarts it after a crash and leaves a clean shutdown alone.
///
/// `ProcessType: Interactive` opts out of launchd's CPU throttling, which applies to the
/// default `Standard` type and would add latency to the one path this project cares about.
pub fn launch_agent_plist(exe: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#,
        label = LAUNCHD_LABEL,
        exe = xml_escape(&exe.display().to_string()),
    )
}

/// The systemd **user** unit written to `$XDG_CONFIG_HOME/systemd/user/`.
///
/// `WantedBy=default.target` rather than `graphical-session.target`: the latter is only reached
/// when the desktop environment explicitly pulls it in, which several do not, and a unit wanted
/// by a target nothing activates is a unit that never starts.
///
/// `Restart=on-failure` and not `always`, for the same reason the launchd `KeepAlive` is a dict:
/// a deliberate stop has to stay stopped.
pub fn systemd_unit(exe: &Path) -> String {
    // Quoted because systemd splits Exec lines on whitespace, and a path under
    // `~/Applications/My Apps/` would otherwise be read as a command plus two arguments.
    format!(
        "[Unit]\n\
         Description=MuxDeck input daemon\n\
         After=graphical-session.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart=\"{exe}\"\n\
         Restart=on-failure\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        exe = exe.display(),
    )
}

/// Escapes the five XML metacharacters.
///
/// A macOS user account named `Ben & Jerry` produces a home directory path containing `&`, which
/// makes the plist malformed XML; launchd then ignores the file and auto-start silently never
/// happens. Rare, but free to prevent.
fn xml_escape(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for c in raw.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// True when `exe` sits somewhere a permanent registration should not point at.
///
/// Every mechanism here stores an absolute path, so registering a `cargo run` build means the
/// next `cargo clean` leaves a logon task pointing at nothing. That is not an error — a
/// developer may well want it — but it is worth saying out loud, because the symptom appears one
/// reboot later with no message.
pub fn is_transient_exe_path(exe: &Path) -> bool {
    if exe.starts_with(std::env::temp_dir()) {
        return true;
    }

    let components: Vec<String> = exe
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();

    components.windows(2).any(|pair| {
        pair[0].eq_ignore_ascii_case("target")
            && (pair[1].eq_ignore_ascii_case("debug") || pair[1].eq_ignore_ascii_case("release"))
    })
}

/// Reads `schtasks /Query /TN MuxDeck /FO CSV /NH` output, whose last column is the task state.
///
/// Empty output means the task exists but printed nothing, which should not happen; treating it
/// as not installed is the safe reading, since the caller only ever reaches here after a
/// zero exit code.
///
/// The state word itself (`Ready`, `Running`, `Disabled`) is localised by Windows, so on a
/// non-English install this reports [`ServiceStatus::InstalledStopped`] for a task that is in
/// fact running. That degrades in the harmless direction: the panel offers Start, a second
/// daemon launches, fails to bind the port and exits. The locale-proof alternatives are all
/// worse — enumerating processes, or a COM call into the Task Scheduler API, which would mean
/// `unsafe` in a crate that forbids it.
pub fn parse_schtasks_status(csv: &str) -> ServiceStatus {
    for line in csv.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let state = line
            .rsplit(',')
            .next()
            .unwrap_or_default()
            .trim()
            .trim_matches('"');
        return if state.eq_ignore_ascii_case("running") {
            ServiceStatus::Running
        } else {
            ServiceStatus::InstalledStopped
        };
    }
    ServiceStatus::NotInstalled
}

/// True when `launchctl list <label>` shows a live process.
///
/// launchd prints a plist-ish dictionary for a loaded job and includes a `"PID"` key only while
/// the job is actually running, so its presence is the distinction between loaded-and-up and
/// loaded-but-idle. Unlike the Windows state column this key is not localised.
pub fn launchctl_reports_a_pid(list_output: &str) -> bool {
    list_output
        .lines()
        .any(|line| line.trim_start().starts_with("\"PID\""))
}

/// True when `systemctl --user is-active` reported `active`.
///
/// The word is part of systemd's machine-readable interface and is never translated. `activating`
/// is deliberately not counted: the socket is not up yet, so reporting Running would have the
/// panel skip the wait it needs to do.
pub fn systemctl_reports_active(is_active_output: &str) -> bool {
    is_active_output.trim() == "active"
}

// ---------------------------------------------------------------------------
// Process plumbing, shared by the three real platform arms.
// ---------------------------------------------------------------------------

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
mod tool {
    use std::ffi::OsStr;
    use std::path::PathBuf;
    use std::process::{Command, Output};

    use super::is_transient_exe_path;
    use crate::error::{EngineError, Result};

    /// The absolute path to register.
    ///
    /// Deliberately not canonicalised. On Windows `canonicalize` returns a `\\?\`-prefixed
    /// extended-length path, which `schtasks` stores verbatim and Task Scheduler then struggles
    /// to launch; `current_exe` already returns an absolute path on every target here.
    pub fn daemon_exe() -> Result<PathBuf> {
        std::env::current_exe().map_err(|e| {
            EngineError::service(
                "locating the muxdeckd binary",
                format!("could not read this process's own path: {e}"),
            )
        })
    }

    /// The warning to show when the binary being registered will not survive a `cargo clean`.
    pub fn transient_exe_note(exe: &std::path::Path) -> Option<String> {
        if !is_transient_exe_path(exe) {
            return None;
        }
        Some(format!(
            "Warning: registered {}, which is a build or temporary directory. The registration \
             stores that exact path and will silently stop working once the directory is cleaned \
             or moved. Re-run `muxdeckd service install` from the installed location.",
            exe.display()
        ))
    }

    /// Runs a platform tool to completion and hands back its raw result.
    ///
    /// A non-zero exit is not an error here: for the query commands it *is* the answer ("no such
    /// task", "inactive"). Only failing to launch the tool at all is a fault.
    pub fn invoke<I, S>(context: &'static str, program: &str, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Command::new(program)
            .args(args)
            .output()
            .map_err(|e| EngineError::service(context, format!("could not run {program}: {e}")))
    }

    /// Turns a non-zero exit into an [`EngineError::Service`] carrying whatever the tool said.
    ///
    /// `schtasks` and `launchctl` both write their diagnostics to stdout as often as to stderr,
    /// hence the fallback — an error reading only "exited with code 1" is useless to the user
    /// standing in front of the panel.
    pub fn require_success(context: &'static str, program: &str, output: &Output) -> Result<()> {
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = if stderr.trim().is_empty() {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        Err(EngineError::service(
            context,
            format!("{program} exited with {}: {detail}", output.status),
        ))
    }
}

// ---------------------------------------------------------------------------
// Windows — Scheduled Task at logon.
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod platform {
    use std::path::Path;

    use super::tool::{daemon_exe, invoke, require_success, transient_exe_note};
    use super::{
        parse_schtasks_status, utf16le_with_bom, windows_task_xml, Installation, ServiceStatus,
        WINDOWS_TASK_FILE, WINDOWS_TASK_NAME,
    };
    use crate::config::write_atomic;
    use crate::error::{EngineError, Result};

    const CONTEXT_CREATE: &str = "creating the MuxDeck scheduled task";
    const CONTEXT_DELETE: &str = "deleting the MuxDeck scheduled task";
    const CONTEXT_QUERY: &str = "querying the MuxDeck scheduled task";

    /// The account the trigger and the principal both name, as `DOMAIN\user`.
    ///
    /// Read from the environment rather than looked up through Win32, for the same reason
    /// [`crate::secret_file`] shells out to `icacls`: this crate forbids `unsafe`, and this runs
    /// once per install. `USERDOMAIN` is the machine name on a workgroup PC, so the qualified
    /// form is correct on domain-joined and standalone machines alike.
    fn current_user() -> Result<String> {
        let user = std::env::var("USERNAME").map_err(|_| {
            EngineError::service(
                CONTEXT_CREATE,
                "USERNAME is not set, so the account to register the task for is unknown",
            )
        })?;
        Ok(match std::env::var("USERDOMAIN") {
            Ok(domain) if !domain.is_empty() => format!("{domain}\\{user}"),
            _ => user,
        })
    }

    pub fn install(config_dir: &Path) -> Result<Installation> {
        let exe = daemon_exe()?;
        let definition = config_dir.join(WINDOWS_TASK_FILE);

        write_atomic(
            &definition,
            &utf16le_with_bom(&windows_task_xml(&exe, &current_user()?)),
        )?;

        // `/F` replaces an existing task rather than failing, which is what makes `install`
        // idempotent. A Windows *Service* would be the wrong thing to register here even if it
        // were easier — session 0 has no desktop to inject into. See the module docs.
        let output = invoke(
            CONTEXT_CREATE,
            "schtasks",
            [
                "/Create".as_ref(),
                "/TN".as_ref(),
                WINDOWS_TASK_NAME.as_ref(),
                "/XML".as_ref(),
                definition.as_os_str(),
                "/F".as_ref(),
            ],
        )?;
        require_success(CONTEXT_CREATE, "schtasks", &output)?;

        Ok(Installation {
            location: format!(
                "scheduled task \"{WINDOWS_TASK_NAME}\", triggered at logon (defined by {})",
                definition.display()
            ),
            notes: transient_exe_note(&exe).into_iter().collect(),
        })
    }

    pub fn uninstall() -> Result<()> {
        // Asking first keeps this idempotent without having to match on `schtasks`'s localised
        // "cannot find the file specified" message.
        if status()? == ServiceStatus::NotInstalled {
            return Ok(());
        }

        let output = invoke(
            CONTEXT_DELETE,
            "schtasks",
            ["/Delete", "/TN", WINDOWS_TASK_NAME, "/F"],
        )?;
        require_success(CONTEXT_DELETE, "schtasks", &output)
    }

    pub fn status() -> Result<ServiceStatus> {
        let output = invoke(
            CONTEXT_QUERY,
            "schtasks",
            ["/Query", "/TN", WINDOWS_TASK_NAME, "/FO", "CSV", "/NH"],
        )?;

        if !output.status.success() {
            return Ok(ServiceStatus::NotInstalled);
        }
        Ok(parse_schtasks_status(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }
}

// ---------------------------------------------------------------------------
// macOS — launchd LaunchAgent.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod platform {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::tool::{daemon_exe, invoke, require_success, transient_exe_note};
    use super::{
        launch_agent_plist, launchctl_reports_a_pid, Installation, ServiceStatus, LAUNCHD_LABEL,
    };
    use crate::config::write_atomic;
    use crate::error::{EngineError, Result};

    const CONTEXT_LOAD: &str = "loading the MuxDeck launch agent";
    const CONTEXT_LIST: &str = "querying the MuxDeck launch agent";

    /// `~/Library/LaunchAgents/in.redoimagined.muxdeck.plist`.
    ///
    /// A LaunchAgent, not a LaunchDaemon: agents run per-user inside the login session, daemons
    /// run as root outside it and could not reach the user's desktop.
    fn plist_path() -> Result<PathBuf> {
        let home = directories::BaseDirs::new().ok_or_else(|| {
            EngineError::service(
                "locating the LaunchAgents directory",
                "no home directory was found for this user",
            )
        })?;
        Ok(home
            .home_dir()
            .join("Library/LaunchAgents")
            .join(format!("{LAUNCHD_LABEL}.plist")))
    }

    pub fn install(_config_dir: &Path) -> Result<Installation> {
        let exe = daemon_exe()?;
        let path = plist_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| EngineError::io("creating the LaunchAgents directory", parent, e))?;
        }

        // Unload any previous copy first: launchd caches the job definition, so overwriting the
        // plist under a loaded label leaves the old command line in force until the next logout.
        let _ = invoke(
            CONTEXT_LOAD,
            "launchctl",
            ["unload".as_ref(), "-w".as_ref(), path.as_os_str()],
        );

        write_atomic(&path, launch_agent_plist(&exe).as_bytes())?;

        // ponytail: `load -w` is the deprecated spelling of `bootstrap gui/<uid>`, kept because
        // it needs no uid lookup and still works on every macOS this project supports. Move to
        // `bootstrap`/`bootout` if a release ever drops it.
        let output = invoke(
            CONTEXT_LOAD,
            "launchctl",
            ["load".as_ref(), "-w".as_ref(), path.as_os_str()],
        )?;
        require_success(CONTEXT_LOAD, "launchctl", &output)?;

        Ok(Installation {
            location: path.display().to_string(),
            notes: transient_exe_note(&exe).into_iter().collect(),
        })
    }

    pub fn uninstall() -> Result<()> {
        let path = plist_path()?;
        if !path.exists() {
            return Ok(());
        }

        // Best-effort: the file is the part that decides whether the agent comes back at the
        // next logon, so a failed unload must not leave it behind. Worst case the daemon keeps
        // running until the session ends, which is visible and recoverable.
        let _ = invoke(
            CONTEXT_LOAD,
            "launchctl",
            ["unload".as_ref(), "-w".as_ref(), path.as_os_str()],
        );

        fs::remove_file(&path)
            .map_err(|e| EngineError::io("removing the launch agent plist", &path, e))
    }

    pub fn status() -> Result<ServiceStatus> {
        let path = plist_path()?;
        if !path.exists() {
            return Ok(ServiceStatus::NotInstalled);
        }

        let output = invoke(CONTEXT_LIST, "launchctl", ["list", LAUNCHD_LABEL])?;
        if !output.status.success() {
            // The plist is on disk but the label is not loaded — installed, waiting for the next
            // logon or an explicit start.
            return Ok(ServiceStatus::InstalledStopped);
        }

        Ok(
            if launchctl_reports_a_pid(&String::from_utf8_lossy(&output.stdout)) {
                ServiceStatus::Running
            } else {
                ServiceStatus::InstalledStopped
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Linux — systemd user unit, plus the udev rule the user must install as root.
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod platform {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::tool::{daemon_exe, invoke, require_success, transient_exe_note};
    use super::{
        systemctl_reports_active, systemd_unit, Installation, ServiceStatus, SYSTEMD_UNIT_NAME,
        UDEV_RULE, UDEV_RULE_FILE,
    };
    use crate::config::write_atomic;
    use crate::error::{EngineError, Result};

    const CONTEXT_ENABLE: &str = "enabling the MuxDeck user unit";
    const CONTEXT_RELOAD: &str = "reloading the systemd user manager";
    const CONTEXT_ACTIVE: &str = "querying the MuxDeck user unit";

    /// `$XDG_CONFIG_HOME/systemd/user/muxdeck.service`, which is where a *user* manager looks —
    /// `/etc/systemd/system/` is the system manager's, needs root, and would start the daemon
    /// outside any login session.
    fn unit_path() -> Result<PathBuf> {
        let dirs = directories::BaseDirs::new().ok_or_else(|| {
            EngineError::service(
                "locating the systemd user unit directory",
                "no home directory was found for this user",
            )
        })?;
        Ok(dirs
            .config_dir()
            .join("systemd/user")
            .join(SYSTEMD_UNIT_NAME))
    }

    pub fn install(config_dir: &Path) -> Result<Installation> {
        let exe = daemon_exe()?;
        let path = unit_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                EngineError::io("creating the systemd user unit directory", parent, e)
            })?;
        }
        write_atomic(&path, systemd_unit(&exe).as_bytes())?;

        // Without this systemd keeps serving its cached view of the unit directory and
        // `enable` fails with "unit not found" on a file that plainly exists.
        let output = invoke(CONTEXT_RELOAD, "systemctl", ["--user", "daemon-reload"])?;
        require_success(CONTEXT_RELOAD, "systemctl", &output)?;

        let output = invoke(
            CONTEXT_ENABLE,
            "systemctl",
            ["--user", "enable", "--now", SYSTEMD_UNIT_NAME],
        )?;
        require_success(CONTEXT_ENABLE, "systemctl", &output)?;

        let rule_path = config_dir.join(UDEV_RULE_FILE);
        write_atomic(&rule_path, UDEV_RULE.as_bytes())?;

        // Printed, never run. Both steps need root, and an installer that silently escalates is
        // one a user is right to distrust — especially one whose whole job is synthesising
        // keystrokes. One note per paragraph; `muxdeckd` separates them with a blank line.
        let rule = rule_path.display();
        let mut notes = vec![
            "Input injection needs write access to /dev/uinput, and granting it needs root, so \
             this command did not do it. The rule has been written out for you; install it with:"
                .to_string(),
            format!(
                "    sudo install -m 644 {rule} /etc/udev/rules.d/{UDEV_RULE_FILE}\n    \
                 sudo udevadm control --reload-rules && sudo udevadm trigger\n    \
                 sudo usermod -aG input \"$USER\""
            ),
            "Then log out and back in. Group membership is only read at login, so until you do \
             every button fails with a permission error even though the rule is already in place."
                .to_string(),
        ];
        notes.extend(transient_exe_note(&exe));

        Ok(Installation {
            location: path.display().to_string(),
            notes,
        })
    }

    pub fn uninstall() -> Result<()> {
        let path = unit_path()?;
        if !path.exists() {
            return Ok(());
        }

        // Best-effort, in that order: removing the file is what actually stops it coming back at
        // the next login, and a machine with no systemd user bus (a container, a session started
        // outside logind) would otherwise fail here and leave the unit installed forever.
        let _ = invoke(
            CONTEXT_ENABLE,
            "systemctl",
            ["--user", "disable", "--now", SYSTEMD_UNIT_NAME],
        );

        fs::remove_file(&path)
            .map_err(|e| EngineError::io("removing the systemd user unit", &path, e))?;

        let _ = invoke(CONTEXT_RELOAD, "systemctl", ["--user", "daemon-reload"]);

        // The udev rule is left alone deliberately. It may already have been copied to
        // /etc/udev/rules.d/ — which this cannot undo without root anyway — and the user's
        // `input` group membership outlives MuxDeck regardless.
        Ok(())
    }

    pub fn status() -> Result<ServiceStatus> {
        if !unit_path()?.exists() {
            return Ok(ServiceStatus::NotInstalled);
        }

        // `is-active` exits non-zero for anything but "active", so the exit code is discarded and
        // only the word it printed is read.
        let output = invoke(
            CONTEXT_ACTIVE,
            "systemctl",
            ["--user", "is-active", SYSTEMD_UNIT_NAME],
        )?;

        Ok(
            if systemctl_reports_active(&String::from_utf8_lossy(&output.stdout)) {
                ServiceStatus::Running
            } else {
                ServiceStatus::InstalledStopped
            },
        )
    }
}

// ---------------------------------------------------------------------------
// Anything else — compiles, refuses at runtime.
//
// CI builds this workspace on windows, macos and ubuntu, so this arm exists only to keep the
// crate buildable on a target nobody has asked for yet (a BSD, say) rather than failing to
// compile the whole engine over a feature that target does not have.
// ---------------------------------------------------------------------------

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
mod platform {
    use std::path::Path;

    use super::{Installation, ServiceStatus};
    use crate::error::{EngineError, Result};

    fn unsupported(context: &'static str) -> EngineError {
        EngineError::service(
            context,
            format!(
                "MuxDeck has no auto-start mechanism for {}; start muxdeckd from your desktop \
                 environment's own autostart configuration instead",
                std::env::consts::OS
            ),
        )
    }

    pub fn install(_config_dir: &Path) -> Result<Installation> {
        Err(unsupported("registering auto-start"))
    }

    pub fn uninstall() -> Result<()> {
        Err(unsupported("removing the auto-start registration"))
    }

    pub fn status() -> Result<ServiceStatus> {
        Ok(ServiceStatus::NotInstalled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Everything here is pure: no test registers a task, writes to ~/Library, or touches
    // systemd. The one round trip that would is #[ignore]d at the bottom.

    #[test]
    fn the_task_is_scoped_to_one_user_and_needs_no_elevation() {
        // Every assertion here is a thing that made `schtasks /Create` fail with "Access is
        // denied." for a non-elevated caller during M5. See windows_task_xml.
        let xml = windows_task_xml(
            Path::new(r"C:\Program Files\MuxDeck\muxdeckd.exe"),
            r"ENIGMA-ENTROPY\cipher",
        );
        assert!(xml.contains("<LogonTrigger>"));
        assert_eq!(
            xml.matches(r"<UserId>ENIGMA-ENTROPY\cipher</UserId>")
                .count(),
            2,
            "the trigger and the principal must both name the user"
        );
        assert!(xml.contains("<LogonType>InteractiveToken</LogonType>"));
        assert!(xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
        assert!(!xml.contains("HighestAvailable"));
    }

    #[test]
    fn the_task_has_no_deadline_and_survives_unplugging_the_laptop() {
        let xml = windows_task_xml(Path::new(r"C:\MuxDeck\muxdeckd.exe"), "PC\\me");
        assert!(
            xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"),
            "the PT72H default terminates the daemon after three days"
        );
        assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(xml.contains("<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>"));
        assert!(xml.contains("<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>"));
        assert!(xml.contains(r"<Command>C:\MuxDeck\muxdeckd.exe</Command>"));
    }

    #[test]
    fn the_task_definition_is_utf16le_with_a_bom() {
        // MSXML rejects the file outright if the bytes and the declaration disagree.
        let bytes = utf16le_with_bom("<?xml version=\"1.0\" encoding=\"UTF-16\"?>");
        assert_eq!(&bytes[..2], &[0xFF, 0xFE]);
        assert_eq!(&bytes[2..6], &[b'<', 0x00, b'?', 0x00]);
        assert!(windows_task_xml(Path::new("x"), "y").contains("encoding=\"UTF-16\""));
    }

    #[test]
    fn the_plist_carries_the_label_the_exe_and_run_at_load() {
        let plist = launch_agent_plist(Path::new("/usr/local/bin/muxdeckd"));
        assert!(plist.contains("<string>in.redoimagined.muxdeck</string>"));
        assert!(plist.contains("<string>/usr/local/bin/muxdeckd</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>\n    <true/>"));
    }

    #[test]
    fn keep_alive_does_not_resurrect_a_clean_shutdown() {
        // If this ever becomes a bare `<true/>`, the panel's "Stop engine" button stops working
        // and the daemon comes back within seconds.
        let plist = launch_agent_plist(Path::new("/usr/local/bin/muxdeckd"));
        assert!(plist.contains("<key>SuccessfulExit</key>\n        <false/>"));
    }

    #[test]
    fn an_ampersand_in_the_path_cannot_break_the_plist() {
        let plist = launch_agent_plist(Path::new("/Users/ben & jerry/muxdeckd"));
        assert!(plist.contains("/Users/ben &amp; jerry/muxdeckd"));
        assert!(
            !plist.contains("ben & jerry"),
            "a raw ampersand makes the plist malformed XML and launchd ignores the file"
        );
    }

    #[test]
    fn the_unit_is_a_user_unit_that_stays_stopped_when_stopped() {
        let unit = systemd_unit(Path::new("/usr/bin/muxdeckd"));
        assert!(unit.contains("ExecStart=\"/usr/bin/muxdeckd\""));
        assert!(unit.contains("Restart=on-failure"));
        assert!(
            unit.contains("WantedBy=default.target"),
            "graphical-session.target is not reached on every desktop"
        );
        assert!(!unit.contains("Restart=always"));
    }

    #[test]
    fn a_path_with_spaces_stays_one_argument_in_the_unit() {
        let unit = systemd_unit(Path::new("/opt/My Apps/muxdeckd"));
        assert!(unit.contains("ExecStart=\"/opt/My Apps/muxdeckd\""));
    }

    #[test]
    fn the_udev_rule_matches_the_spec_verbatim() {
        assert!(UDEV_RULE.contains(r#"KERNEL=="uinput", GROUP="input", MODE="0660""#));
    }

    #[test]
    fn schtasks_running_and_ready_are_distinguished() {
        assert_eq!(
            parse_schtasks_status("\"MuxDeck\",\"N/A\",\"Running\"\r\n"),
            ServiceStatus::Running
        );
        assert_eq!(
            parse_schtasks_status("\"MuxDeck\",\"N/A\",\"Ready\"\r\n"),
            ServiceStatus::InstalledStopped
        );
        assert_eq!(
            parse_schtasks_status("\"MuxDeck\",\"N/A\",\"Disabled\"\r\n"),
            ServiceStatus::InstalledStopped
        );
    }

    #[test]
    fn schtasks_blank_lines_are_skipped_and_empty_output_is_not_installed() {
        assert_eq!(
            parse_schtasks_status("\r\n\r\n\"MuxDeck\",\"N/A\",\"Running\"\r\n"),
            ServiceStatus::Running
        );
        assert_eq!(parse_schtasks_status(""), ServiceStatus::NotInstalled);
        assert_eq!(parse_schtasks_status("  \n"), ServiceStatus::NotInstalled);
    }

    #[test]
    fn a_localised_state_word_degrades_to_installed_not_running() {
        // German Windows prints "Bereit"/"Wird ausgeführt". Neither matches, and the safe
        // reading is the stopped one — see parse_schtasks_status.
        assert_eq!(
            parse_schtasks_status("\"MuxDeck\",\"N/A\",\"Bereit\"\r\n"),
            ServiceStatus::InstalledStopped
        );
    }

    #[test]
    fn launchctl_pid_means_running() {
        let loaded_and_up = "{\n\t\"LimitLoadToSessionType\" = \"Aqua\";\n\t\"Label\" = \
                             \"in.redoimagined.muxdeck\";\n\t\"OnDemand\" = false;\n\t\"PID\" = \
                             4242;\n\t\"LastExitStatus\" = 0;\n};\n";
        assert!(launchctl_reports_a_pid(loaded_and_up));

        let loaded_but_idle = "{\n\t\"Label\" = \"in.redoimagined.muxdeck\";\n\t\
                               \"LastExitStatus\" = 0;\n};\n";
        assert!(!launchctl_reports_a_pid(loaded_but_idle));
    }

    #[test]
    fn systemctl_counts_only_active_as_running() {
        assert!(systemctl_reports_active("active\n"));
        assert!(!systemctl_reports_active("inactive\n"));
        assert!(!systemctl_reports_active("failed\n"));
        assert!(
            !systemctl_reports_active("activating\n"),
            "the socket is not up during activating; the panel still needs to wait"
        );
    }

    #[test]
    fn a_build_directory_is_recognised_as_transient() {
        assert!(is_transient_exe_path(Path::new(
            "/home/dev/muxdeck/engine/target/debug/muxdeckd"
        )));
        assert!(is_transient_exe_path(Path::new(
            r"F:\projects\muxdeck\engine\target\release\muxdeckd.exe"
        )));
        assert!(is_transient_exe_path(Path::new(
            "/home/dev/muxdeck/engine/target/debug/deps/muxdeck_engine-abc123"
        )));
        assert!(is_transient_exe_path(
            &std::env::temp_dir().join("muxdeckd")
        ));
    }

    #[test]
    fn an_installed_location_is_not_transient() {
        assert!(!is_transient_exe_path(Path::new("/usr/local/bin/muxdeckd")));
        assert!(!is_transient_exe_path(Path::new(
            r"C:\Program Files\MuxDeck\muxdeckd.exe"
        )));
        assert!(
            !is_transient_exe_path(Path::new("/opt/targets/debug-tools/muxdeckd")),
            "only a literal target/debug or target/release pair counts"
        );
    }

    #[test]
    fn status_answers_without_a_registration_present() {
        // Read-only on every platform, and safe on a CI runner: with nothing installed each arm
        // short-circuits before it needs a task scheduler, a launchd session or a user bus.
        assert!(status().is_ok());
    }

    #[test]
    #[ignore = "registers real auto-start with the OS: creates a scheduled task, LaunchAgent or systemd user unit"]
    fn install_status_uninstall_round_trip() {
        let config_dir = std::env::temp_dir().join("muxdeck-service-tests");
        std::fs::create_dir_all(&config_dir).expect("temp config dir");

        let installed = install(&config_dir).expect("install");
        assert!(!installed.location.is_empty());
        assert_ne!(status().expect("status"), ServiceStatus::NotInstalled);

        uninstall().expect("uninstall");
        assert_eq!(status().expect("status"), ServiceStatus::NotInstalled);

        uninstall().expect("uninstalling twice must be a no-op, not an error");
        let _ = std::fs::remove_dir_all(&config_dir);
    }
}
