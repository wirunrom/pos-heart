//! Receipt printing on ESC/POS thermal printers (58 / 80 mm).
//! Settings are per device in `printer.json` beside the database (not backed
//! up with sales). Jobs render and send on their own thread; a failed print
//! never affects the sale, which is already committed.
pub mod docs;
pub mod escpos;
pub mod render;
pub mod template;

use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Connection {
    /// No printer: print actions are hidden.
    #[default]
    None,
    /// Network printer (raw TCP, usually port 9100).
    Network,
    /// A printer queue of the operating system, sent raw (macOS / Linux CUPS).
    System,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Paper {
    #[serde(rename = "58mm")]
    Mm58,
    #[default]
    #[serde(rename = "80mm")]
    Mm80,
}
impl Paper {
    /// Printable dots at 203 dpi.
    pub fn dots(self) -> u32 {
        match self {
            Self::Mm58 => 384,
            Self::Mm80 => 576,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrinterSettings {
    pub version: u32,
    pub connection: Connection,
    /// Network address: IP or host name, optional `:port` (default 9100).
    #[serde(default)]
    pub address: String,
    /// System queue name (`lpstat -a`).
    #[serde(default)]
    pub queue: String,
    pub paper: Paper,
    /// Print the receipt as soon as a payment is recorded.
    pub auto_print: bool,
    /// Pulse the cash drawer (connected to the printer) on cash payments.
    pub open_drawer: bool,
    /// Text printed under the receipt, e.g. a thank-you line.
    #[serde(default)]
    pub footer: String,
    /// Where kitchen tickets print.
    #[serde(default)]
    pub kitchen: KitchenPrinter,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KitchenMode {
    /// Kitchen tickets are not printed (the kitchen screen still shows them).
    #[default]
    Off,
    /// On the receipt printer.
    Receipt,
    /// On a separate printer in the kitchen.
    Own,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KitchenPrinter {
    pub mode: KitchenMode,
    #[serde(default)]
    pub connection: Connection,
    #[serde(default)]
    pub address: String,
    #[serde(default)]
    pub queue: String,
    #[serde(default)]
    pub paper: Paper,
}
impl Default for PrinterSettings {
    fn default() -> Self {
        Self {
            version: 1,
            connection: Connection::None,
            address: String::new(),
            queue: String::new(),
            paper: Paper::Mm80,
            auto_print: false,
            open_drawer: false,
            footer: String::new(),
            kitchen: KitchenPrinter::default(),
        }
    }
}
impl PrinterSettings {
    const FILE: &'static str = "printer.json";
    pub fn load(dir: &Path) -> io::Result<Self> {
        let bytes = match fs::read(dir.join(Self::FILE)) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e),
        };
        let s: Self = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        if s.version != 1 {
            return Err(io::Error::other("Unsupported printer settings version"));
        }
        Ok(s)
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.footer.chars().count() > 200 {
            return Err("footer");
        }
        if self.kitchen.mode == KitchenMode::Own {
            device_valid(
                self.kitchen.connection,
                &self.kitchen.address,
                &self.kitchen.queue,
            )?;
            if self.kitchen.connection == Connection::None {
                return Err("kitchen");
            }
        }
        device_valid(self.connection, &self.address, &self.queue)
    }
    /// Settings that print kitchen tickets, if any.
    pub fn kitchen_device(&self) -> Option<PrinterSettings> {
        let device = match self.kitchen.mode {
            KitchenMode::Off => return None,
            KitchenMode::Receipt => self.clone(),
            KitchenMode::Own => PrinterSettings {
                connection: self.kitchen.connection,
                address: self.kitchen.address.clone(),
                queue: self.kitchen.queue.clone(),
                paper: self.kitchen.paper,
                ..self.clone()
            },
        };
        device.enabled().then_some(device)
    }
    pub fn save(&self, dir: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        let temp = dir.join(format!(".printer-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            f.write_all(&bytes)?;
            f.sync_all()?;
            fs::rename(&temp, dir.join(Self::FILE))
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
    pub fn enabled(&self) -> bool {
        self.connection != Connection::None && self.validate().is_ok()
    }
}
fn device_valid(connection: Connection, address: &str, queue: &str) -> Result<(), &'static str> {
    match connection {
        Connection::None => Ok(()),
        Connection::Network => parse_address(address).map(|_| ()),
        Connection::System if queue.trim().is_empty() => Err("queue"),
        Connection::System => Ok(()),
    }
}
/// `192.168.1.50` or `printer.local:9100`.
pub fn parse_address(text: &str) -> Result<(String, u16), &'static str> {
    let text = text.trim();
    if text.is_empty() || text.chars().any(char::is_whitespace) || text.len() > 253 {
        return Err("address");
    }
    match text.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && !host.contains(':') => {
            let port = port.parse::<u16>().map_err(|_| "address")?;
            if port == 0 {
                return Err("address");
            }
            Ok((host.to_owned(), port))
        }
        Some(_) => Err("address"),
        None => Ok((text.to_owned(), 9100)),
    }
}

/// What to print. Documents are built on the UI thread from committed data.
pub struct Job {
    pub doc: Option<render::Doc>,
    /// Pulse the drawer (cash payments, or "open drawer" alone).
    pub drawer: bool,
    /// Short description for the result toast, e.g. "#000056".
    pub label: String,
}
pub enum Outcome {
    Printed(String),
    Failed { label: String, error: String },
}

/// Renders and sends `job` on a new thread; the result arrives on the channel.
pub fn spawn(
    settings: PrinterSettings,
    job: Job,
    ctx: eframe::egui::Context,
) -> mpsc::Receiver<Outcome> {
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("print".into())
        .spawn(move || {
            let label = job.label.clone();
            let outcome = match send(&settings, &job) {
                Ok(()) => Outcome::Printed(label),
                Err(e) => Outcome::Failed {
                    label,
                    error: e.to_string(),
                },
            };
            let _ = tx.send(outcome);
            ctx.request_repaint();
        });
    if let Err(e) = spawned {
        let (tx, rx) = mpsc::channel();
        let _ = tx.send(Outcome::Failed {
            label: String::new(),
            error: e.to_string(),
        });
        return rx;
    }
    rx
}
fn bytes(settings: &PrinterSettings, job: &Job) -> io::Result<Vec<u8>> {
    match &job.doc {
        Some(doc) => {
            let img = render::render(doc, settings.paper.dots())
                .ok_or_else(|| io::Error::other("fonts"))?;
            Ok(escpos::encode(&img, true, job.drawer))
        }
        None => {
            let mut out = vec![0x1B, b'@'];
            out.extend(escpos::drawer_pulse());
            Ok(out)
        }
    }
}
fn send(settings: &PrinterSettings, job: &Job) -> io::Result<()> {
    let data = bytes(settings, job)?;
    match settings.connection {
        Connection::None => Err(io::Error::other("no printer")),
        Connection::Network => {
            let (host, port) = parse_address(&settings.address).map_err(io::Error::other)?;
            let addr = (host.as_str(), port)
                .to_socket_addrs()?
                .next()
                .ok_or_else(|| io::Error::other("address not found"))?;
            let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(4))?;
            stream.set_write_timeout(Some(Duration::from_secs(8)))?;
            stream.write_all(&data)?;
            stream.flush()
        }
        Connection::System => system_print(&settings.queue, &data),
    }
}
#[cfg(unix)]
fn system_print(queue: &str, data: &[u8]) -> io::Result<()> {
    use std::process::{Command, Stdio};
    let mut child = Command::new("lp")
        .args(["-s", "-d", queue.trim(), "-o", "raw"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(data)?;
    }
    let out = child.wait_with_output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(
            String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        ))
    }
}
#[cfg(not(unix))]
fn system_print(_queue: &str, _data: &[u8]) -> io::Result<()> {
    Err(io::Error::other(
        "System printer queues are not supported on this platform yet; use a network printer",
    ))
}
/// Printer queues known to the operating system (CUPS `lpstat -a`).
#[cfg(unix)]
pub fn system_queues() -> Vec<String> {
    let Ok(out) = std::process::Command::new("lpstat").arg("-a").output() else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split_whitespace().next().map(str::to_owned))
        .collect()
}
#[cfg(not(unix))]
pub fn system_queues() -> Vec<String> {
    Vec::new()
}
pub fn system_supported() -> bool {
    cfg!(unix)
}
/// Writes the rendered document as PNG so the layout can be checked without a printer.
pub fn save_preview(
    settings: &PrinterSettings,
    doc: &render::Doc,
    path: &PathBuf,
) -> io::Result<()> {
    let img =
        render::render(doc, settings.paper.dots()).ok_or_else(|| io::Error::other("fonts"))?;
    img.save(path).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn addresses_default_to_port_9100() {
        assert_eq!(
            parse_address("192.168.1.50"),
            Ok(("192.168.1.50".into(), 9100))
        );
        assert_eq!(
            parse_address(" printer.local:9101 "),
            Ok(("printer.local".into(), 9101))
        );
        for bad in [
            "",
            "a b",
            "host:",
            "host:0",
            "host:99999",
            ":9100",
            "fe80::1",
        ] {
            assert!(parse_address(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn settings_round_trip_and_refuse_future_versions() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        assert_eq!(
            PrinterSettings::load(dir.path())?,
            PrinterSettings::default()
        );
        let s = PrinterSettings {
            connection: Connection::Network,
            address: "10.0.0.9".into(),
            paper: Paper::Mm58,
            auto_print: true,
            ..Default::default()
        };
        assert!(s.enabled());
        s.save(dir.path())?;
        assert_eq!(PrinterSettings::load(dir.path())?, s);
        fs::write(
            dir.path().join("printer.json"),
            r#"{"version":2,"connection":"none","paper":"80mm","auto_print":false,"open_drawer":false}"#,
        )?;
        assert!(PrinterSettings::load(dir.path()).is_err());
        Ok(())
    }
    #[test]
    fn network_jobs_reach_a_listening_printer() -> Result<(), Box<dyn std::error::Error>> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let reader = std::thread::spawn(move || -> io::Result<Vec<u8>> {
            let (mut s, _) = listener.accept()?;
            let mut buf = Vec::new();
            io::Read::read_to_end(&mut s, &mut buf)?;
            Ok(buf)
        });
        let settings = PrinterSettings {
            connection: Connection::Network,
            address: format!("127.0.0.1:{port}"),
            paper: Paper::Mm58,
            ..Default::default()
        };
        let mut doc = render::Doc::default();
        doc.text("ทดสอบ", render::Style::Title, render::Align::Center);
        send(
            &settings,
            &Job {
                doc: Some(doc),
                drawer: false,
                label: String::new(),
            },
        )?;
        let got = reader.join().map_err(|_| "reader")??;
        assert!(got.starts_with(&[0x1B, b'@', 0x1D, b'v', b'0', 0, 48, 0]));
        Ok(())
    }
}
