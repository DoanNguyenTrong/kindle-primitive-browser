//! Diagnostics: a fixed list of read-only checks, and two fixed DNS changes, all run through the shell engine
//! (`kb-term`) and shown on the `kbp://diagnostics` page. The commands are written here; nothing the user
//! types reaches the shell.

use std::time::{Duration, Instant};

use kb_term::{Output, Session, SessionConfig};

use super::*;
use crate::ui::Report;

/// The checks, in the order they are shown: a title and the command. Read-only commands only.
pub(crate) const CHECKS: &[(&str, &str)] = &[
    ("Shell engine", "echo ok"),
    ("System", "uname -a"),
    ("Storage", "df -h /mnt/us 2>&1"),
    ("Network interfaces", "ifconfig 2>&1 | head -n 24"),
    ("DNS servers", "cat /etc/resolv.conf 2>&1"),
    ("Hosts file", "cat /etc/hosts 2>&1"),
    ("Browser files", "ls -l /mnt/us/browser 2>&1"),
    (
        "SSH server",
        "if [ -f /tmp/dropbear_koreader.pid ] && kill -0 $(cat /tmp/dropbear_koreader.pid) 2>/dev/null; then echo \"running, pid $(cat /tmp/dropbear_koreader.pid), port 2222\"; else echo 'not running'; fi",
    ),
    // /proc lists every process with its command line; busybox ps did not show the browser on the device.
    (
        "Browser processes",
        "for p in /proc/[0-9]*; do c=$(tr '\\0' ' ' 2>/dev/null < \"$p/cmdline\"); case \"$c\" in *kb-kindle*) echo \"${p#/proc/} $c\";; esac; done",
    ),
];

/// Time allowed for each check. A check still running after this is stopped with Ctrl-C.
pub(crate) const CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// Most characters of one check's output that are shown. The rest is cut, with a note.
const MAX_OUTPUT_CHARS: usize = 1500;

/// A change made from the diagnostics page. Each is a fixed command. Nothing the user types reaches the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Change {
    /// Cloudflare's and Google's resolvers. The network's original settings are saved to `/mnt/us` first.
    PublicDns,
    /// The network's original settings, as saved by `PublicDns`.
    RestoreDns,
    /// SSH server for remote access on port 2222, with the root password set on the device.
    StartSsh,
    /// Stops the SSH server and every session it started.
    StopSsh,
    /// Reads the SSH server's state: its process, whether port 2222 is listening, and the connect command.
    CheckSsh,
}

/// Time allowed for a change. The copies are small, so this is generous.
const CHANGE_TIMEOUT: Duration = Duration::from_secs(10);

impl Change {
    pub(crate) fn title(self) -> &'static str {
        match self {
            Change::PublicDns => "Set public DNS",
            Change::RestoreDns => "Restore DNS",
            Change::StartSsh => "Start SSH",
            Change::StopSsh => "Stop SSH",
            Change::CheckSsh => "Check SSH",
        }
    }

    /// The shell command. On a machine without `/mnt/us` (the Mac, in tests) nothing is changed.
    pub(crate) fn command(self) -> &'static str {
        match self {
            // The DNS changes take the same steps as the Kindle's set-dns.sh: the system partition is made
            // writable for the copy and read-only again afterwards.
            // Public DNS also patches the Wi-Fi DHCP script, so the setting survives reconnects and restarts.
            // The original script is saved once to /mnt/us/browser/dns; Restore DNS puts it back.
            Change::PublicDns => concat!(
                "if [ ! -d /mnt/us ]; then echo 'not a Kindle: nothing changed'; ",
                "elif [ ! -f /mnt/us/resolv.conf.original ] && ! cp /etc/resolv.conf /mnt/us/resolv.conf.original; then ",
                "echo 'could not save the original DNS settings: nothing changed'; ",
                "else printf 'nameserver 1.1.1.1\\nnameserver 8.8.8.8\\n' > /mnt/us/resolv.conf.new && ",
                "mount -o remount,rw / && { cp /mnt/us/resolv.conf.new /etc/resolv.conf; cp /mnt/us/resolv.conf.new /tmp/resolv.conf; ",
                "mkdir -p /mnt/us/browser/dns; ",
                "[ -f /mnt/us/browser/dns/default.script.original ] || cp /usr/share/udhcpc/default.script /mnt/us/browser/dns/default.script.original; ",
                "sed 's/for i in $dns ; do/for i in 1.1.1.1 8.8.8.8 ; do/' /usr/share/udhcpc/default.script > /tmp/default.script.new && ",
                "grep -q 'for i in 1.1.1.1 8.8.8.8' /tmp/default.script.new && cat /tmp/default.script.new > /usr/share/udhcpc/default.script; ",
                "r=$?; rm -f /tmp/default.script.new; sync; mount -o remount,ro /; echo \"copy status $r\"; }; ",
                "rm -f /mnt/us/resolv.conf.new; fi; ",
                "id -u; cat /etc/resolv.conf"
            ),
            Change::RestoreDns => concat!(
                "if [ ! -d /mnt/us ]; then echo 'not a Kindle: nothing changed'; ",
                "elif [ ! -f /mnt/us/resolv.conf.original ]; then echo 'no saved original DNS settings: nothing changed'; ",
                "else mount -o remount,rw / && { cp /mnt/us/resolv.conf.original /etc/resolv.conf; ",
                "[ -f /mnt/us/browser/dns/default.script.original ] && cp /mnt/us/browser/dns/default.script.original /usr/share/udhcpc/default.script; ",
                "r=$?; sync; mount -o remount,ro /; echo \"copy status $r\"; }; fi; ",
                "id -u; cat /etc/resolv.conf"
            ),
            // Starts the server with the root password already set on the device (set once, with `passwd`).
            // Nothing here changes the system: the password file is left alone.
            Change::StartSsh => concat!(
                "if [ ! -d /mnt/us ]; then echo 'not a Kindle: nothing changed'; ",
                "elif [ -f /tmp/dropbear_koreader.pid ] && kill -0 $(cat /tmp/dropbear_koreader.pid) 2>/dev/null; then ",
                "echo \"already running, pid $(cat /tmp/dropbear_koreader.pid)\"; ",
                "else mkdir -p /mnt/us/browser/ssh; [ $(wc -c < /mnt/us/browser/ssh/dropbear.log 2>/dev/null || echo 0) -gt 262144 ] && mv /mnt/us/browser/ssh/dropbear.log /mnt/us/browser/ssh/dropbear.log.old; ",
                "echo \"started $(date)\" >> /mnt/us/browser/ssh/dropbear.log; ",
                "cd /mnt/us/koreader && ./dropbear -E -R -p 2222 -P /tmp/dropbear_koreader.pid 2>>/mnt/us/browser/ssh/dropbear.log; ",
                "echo \"dropbear status $?\"; sleep 1; ",
                "if [ -f /tmp/dropbear_koreader.pid ]; then echo \"running, pid $(cat /tmp/dropbear_koreader.pid), port 2222\"; fi; fi; ",
                "ip=$(ifconfig wlan0 2>/dev/null | grep -o 'inet addr:[0-9.]*' | cut -d: -f2); ",
                "echo \"ssh -p 2222 root@${ip:-no-address}\""
            ),
            // Read only: the server process, the listening socket (port 2222 is 0x08AE in /proc/net/tcp, state 0A),
            // and the address to connect to.
            Change::CheckSsh => concat!(
                "if [ ! -d /mnt/us ]; then echo 'not a Kindle: nothing to check'; ",
                "else pid=$(cat /tmp/dropbear_koreader.pid 2>/dev/null); ",
                "if [ -n \"$pid\" ] && kill -0 $pid 2>/dev/null; then echo \"server process: running, pid $pid\"; else echo 'server process: not running'; fi; ",
                "if grep -i ':08AE ' /proc/net/tcp 2>/dev/null | grep -q ' 0A '; then echo 'port 2222: listening'; else echo 'port 2222: not listening'; fi; ",
                "ip=$(ifconfig wlan0 2>/dev/null | grep -o 'inet addr:[0-9.]*' | cut -d: -f2); ",
                "echo \"ssh -p 2222 root@${ip:-no-address}\"; ",
                "echo 'last SSH log lines:'; tail -n 8 /mnt/us/browser/ssh/dropbear.log 2>/dev/null || echo '(no SSH log yet: start SSH once)'; fi"
            ),
            // Stops the listener and every session it started (their command lines name dropbear).
            Change::StopSsh => concat!(
                "if [ ! -d /mnt/us ]; then echo 'not a Kindle: nothing changed'; ",
                "elif [ ! -f /tmp/dropbear_koreader.pid ]; then echo 'SSH server is not running'; ",
                "else for p in /proc/[0-9]*; do c=$(tr '\\0' ' ' 2>/dev/null < \"$p/cmdline\"); ",
                "case \"$c\" in *dropbear*) kill ${p#/proc/} 2>/dev/null;; esac; done; ",
                "sleep 1; rm -f /tmp/dropbear_koreader.pid; echo 'SSH server stopped'; fi"
            ),
        }
    }
}

/// Whether the SSH server is running: its pid file names a process that still exists.
pub(crate) fn ssh_running() -> bool {
    std::fs::read_to_string("/tmp/dropbear_koreader.pid")
        .ok()
        .and_then(|pid| pid.trim().parse::<u32>().ok())
        .is_some_and(|pid| std::path::Path::new(&format!("/proc/{pid}")).exists())
}

impl Browser {
    /// Start the SSH server if it is stopped, stop it if it is running.
    pub(super) fn toggle_ssh(&mut self) {
        let change = if ssh_running() { Change::StopSsh } else { Change::StartSsh };
        self.run_change(change);
    }

    /// Run the checks, then show the diagnostics page with their results.
    pub(super) fn run_diagnostics(&mut self) {
        self.diagnostics = Some(run_checks(CHECKS, CHECK_TIMEOUT));
        self.open("kbp://diagnostics");
    }

    /// Make a change, then show the diagnostics page with what it printed.
    pub(super) fn run_change(&mut self, change: Change) {
        self.fix_report = Some(run_fixed(change.title(), change.command(), change.command(), CHANGE_TIMEOUT));
        self.open("kbp://diagnostics");
    }
}

/// Run one fixed command in a new shell and report its output, logged as `[fix]` lines.
pub(crate) fn run_fixed(title: &str, shown: &str, command: &str, timeout: Duration) -> Report {
    let config = SessionConfig {
        shell: "/bin/sh".into(),
        ..SessionConfig::default()
    };
    let output = match Session::spawn(&config) {
        Err(err) => {
            eprintln!("[fix] the shell did not start: {err}");
            format!("The shell did not start: {err}")
        }
        Ok(mut session) => match session.run(command, timeout) {
            Ok(out) => {
                eprintln!("[fix] {title}: status {:?}, timed out {}", out.status, out.timed_out);
                let text = summary(&out);
                for line in text.lines() {
                    // The SSH password is shown on screen only, not written to the log.
                    if line.starts_with("password: ") {
                        eprintln!("[fix]   password: (shown on screen)");
                    } else {
                        eprintln!("[fix]   {line}");
                    }
                }
                text
            }
            Err(err) => {
                eprintln!("[fix] {title}: could not run: {err}");
                format!("Could not run this change: {err}")
            }
        },
    };
    Report {
        title: title.into(),
        command: shown.into(),
        output,
    }
}

/// Whether `pw` is an acceptable SSH password: 6 to 64 letters, digits, or `- _ . @ # % + = !`. Only these
/// characters can reach the shell command, so nothing in the password can change what the command does.
pub(crate) fn valid_ssh_password(pw: &str) -> bool {
    (6..=64).contains(&pw.chars().count())
        && pw.chars().all(|c| c.is_ascii_alphanumeric() || "-_.@#%+=!".contains(c))
}

impl Browser {
    /// Open the keyboard to type the SSH password. The text goes to `set_ssh_password` when Go is tapped.
    pub(super) fn begin_ssh_password(&mut self) {
        self.password_entry = true;
        self.keyboard = true;
        self.sheet = None;
        // The field starts empty: the password is never kept in the source.
        self.typed = Vec::new();
        self.caret = 0;
        self.select_all = false;
        self.layer = Layer::Lower;
        self.refresh_suggestions();
        self.touch(0, self.height, Wave::Text);
    }

    /// Set the root password the SSH server checks, then show the result on the diagnostics page. The
    /// password is written into the command only after it has been checked, and the page shows the command
    /// without it.
    pub(super) fn set_ssh_password(&mut self, pw: &str) {
        if !valid_ssh_password(pw) {
            self.fix_report = Some(Report {
                title: "Set SSH password".into(),
                command: String::new(),
                output: "Use 6 to 64 letters, digits, or - _ . @ # % + = ! (no spaces).".into(),
            });
            self.open("kbp://diagnostics");
            return;
        }
        let command = format!(
            "if [ ! -d /mnt/us ]; then echo 'not a Kindle: nothing changed'; \
             elif ! mount -o remount,rw /; then echo 'could not make / writable: nothing changed'; \
             else echo 'root:{pw}' | chpasswd; r=$?; sync; \
             mount -o remount,ro / 2>/dev/null || echo 'system partition stays writable until restart'; \
             if [ $r -eq 0 ]; then echo 'password set for root'; else echo 'could not set the password'; fi; fi"
        );
        self.fix_report = Some(run_fixed(
            "Set SSH password",
            "Set the root password for SSH (the password is not shown here)",
            &command,
            CHANGE_TIMEOUT,
        ));
        self.open("kbp://diagnostics");
    }
}

/// Run each check in one shell session and report what it printed. If the shell cannot start, the single
/// report says why.
pub(crate) fn run_checks(checks: &[(&str, &str)], timeout: Duration) -> Vec<Report> {
    let started = Instant::now();
    let config = SessionConfig {
        shell: "/bin/sh".into(),
        ..SessionConfig::default()
    };
    let mut session = match Session::spawn(&config) {
        Ok(session) => session,
        Err(err) => {
            eprintln!("[diag] the shell did not start: {err}");
            return vec![Report {
                title: "Shell engine".into(),
                command: "/bin/sh".into(),
                output: format!("The shell did not start: {err}"),
            }];
        }
    };
    eprintln!("[diag] shell started in {} ms", started.elapsed().as_millis());

    checks
        .iter()
        .map(|(title, command)| {
            let output = match session.run(command, timeout) {
                Ok(out) => {
                    eprintln!("[diag] {title}: status {:?}, timed out {}", out.status, out.timed_out);
                    let text = summary(&out);
                    for line in text.lines() {
                        eprintln!("[diag]   {line}");
                    }
                    text
                }
                Err(err) => {
                    eprintln!("[diag] {title}: could not run: {err}");
                    format!("Could not run this check: {err}")
                }
            };
            Report {
                title: (*title).into(),
                command: (*command).into(),
                output,
            }
        })
        .collect()
}

/// The text shown for one check: its output (cut to a length the page can hold), then a note when the
/// check was stopped or exited with a failure status.
pub(crate) fn summary(out: &Output) -> String {
    // The page font has no glyph for a tab, so tabs are shown as spaces.
    let mut text = out.text.trim_end().replace('\t', "    ");
    if text.chars().count() > MAX_OUTPUT_CHARS {
        text = text.chars().take(MAX_OUTPUT_CHARS).collect::<String>() + "\n… (cut short)";
    }
    if text.is_empty() {
        text = "(no output)".into();
    }
    if out.timed_out {
        text.push_str("\n\nStopped: still running after the time limit.");
    } else if let Some(status) = out.status.filter(|s| *s != 0) {
        text.push_str(&format!("\n\nExit status {status}."));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_passwords_must_be_safe_for_the_shell_command() {
        assert!(valid_ssh_password("Kindle-2026_pw"));
        assert!(valid_ssh_password("a@b#c%d+e=f!"));
        assert!(valid_ssh_password("Example-1"));
        assert!(!valid_ssh_password("short"));
        assert!(!valid_ssh_password(&"x".repeat(65)));
        assert!(!valid_ssh_password("has space"));
        assert!(!valid_ssh_password("quote'inside"));
        assert!(!valid_ssh_password("dollar$sign"));
        assert!(!valid_ssh_password("semi;colon1"));
        assert!(!valid_ssh_password("back`tick`1"));
    }

    #[test]
    fn dns_changes_refuse_to_run_off_the_kindle() {
        // Both commands check for /mnt/us first, so on a machine without it the system is not touched.
        for change in [Change::PublicDns, Change::RestoreDns, Change::StartSsh] {
            assert!(change.command().starts_with("if [ ! -d /mnt/us ]; then"), "{change:?}");
        }
        assert!(Change::PublicDns.command().contains("nameserver 1.1.1.1"));
        assert!(Change::PublicDns.command().contains("nameserver 8.8.8.8"));
    }

    #[test]
    fn a_dns_change_off_the_kindle_reports_that_nothing_changed() {
        if std::path::Path::new("/mnt/us").exists() {
            return; // only meaningful off the Kindle
        }
        let report = run_fixed("Set public DNS", Change::PublicDns.command(), Change::PublicDns.command(), Duration::from_secs(5));
        assert!(report.output.starts_with("not a Kindle: nothing changed"), "{}", report.output);
    }

    #[test]
    fn summary_shows_output_and_notes_failures() {
        let ok = Output { text: "hello\n".into(), status: Some(0), timed_out: false };
        assert_eq!(summary(&ok), "hello");
        let tabbed = Output { text: "a\tb".into(), status: Some(0), timed_out: false };
        assert_eq!(summary(&tabbed), "a    b");

        let empty = Output { text: String::new(), status: Some(1), timed_out: false };
        assert_eq!(summary(&empty), "(no output)\n\nExit status 1.");

        let slow = Output { text: "partial".into(), status: None, timed_out: true };
        assert_eq!(summary(&slow), "partial\n\nStopped: still running after the time limit.");
    }

    #[test]
    fn summary_cuts_long_output() {
        let long = Output { text: "x".repeat(MAX_OUTPUT_CHARS + 10), status: Some(0), timed_out: false };
        let shown = summary(&long);
        assert!(shown.ends_with("(cut short)"));
        assert_eq!(shown.chars().filter(|c| *c == 'x').count(), MAX_OUTPUT_CHARS);
    }

    #[test]
    fn checks_run_through_the_shell_engine() {
        let reports = run_checks(
            &[("Shell engine", "echo ok"), ("Exit", "sh -c 'exit 4'"), ("Slow", "sleep 30")],
            Duration::from_millis(800),
        );
        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0].output, "ok");
        assert!(reports[1].output.ends_with("Exit status 4."));
        assert!(reports[2].output.contains("Stopped: still running"));
    }
}
