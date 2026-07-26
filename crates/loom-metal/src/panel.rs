use std::{
    collections::BTreeMap,
    env,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{RuntimeDiagnostic, RuntimeDiagnosticCode};

static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct ProjectUi {
    pub asset_root: PathBuf,
    pub entry: String,
    pub title: String,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct PanelControl {
    pub(crate) name: String,
    pub(crate) value: f32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PanelToViewer {
    Hello { token: String },
    Set { name: String, value: f32 },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ViewerToPanel<'a> {
    Snapshot { values: &'a BTreeMap<String, f32> },
}

pub(crate) struct PanelBridge {
    controls: Receiver<PanelControl>,
    writer: Arc<Mutex<Option<TcpStream>>>,
    child: Child,
    last_publish: Instant,
}

impl PanelBridge {
    pub(crate) fn launch(ui: ProjectUi) -> Result<Self, RuntimeDiagnostic> {
        if !ui.asset_root.is_dir() {
            return Err(panel_error(format!(
                "project panel asset root `{}` does not exist",
                ui.asset_root.display()
            )));
        }
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|error| panel_error(format!("could not bind panel bridge: {error}")))?;
        let address = listener
            .local_addr()
            .map_err(|error| panel_error(format!("could not inspect panel bridge: {error}")))?;
        let token = session_token();
        let (control_sender, controls) = mpsc::channel();
        let writer = Arc::new(Mutex::new(None));
        accept_panel(listener, token.clone(), control_sender, Arc::clone(&writer));

        let executable = panel_executable()?;
        let child = Command::new(&executable)
            .args([
                "--address",
                &address.to_string(),
                "--token",
                &token,
                "--root",
                &ui.asset_root.to_string_lossy(),
                "--entry",
                &ui.entry,
                "--title",
                &ui.title,
                "--width",
                &ui.width.to_string(),
                "--height",
                &ui.height.to_string(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| {
                panel_error(format!(
                    "could not launch project panel `{}`: {error}",
                    executable.display()
                ))
            })?;

        Ok(Self {
            controls,
            writer,
            child,
            last_publish: Instant::now() - Duration::from_secs(1),
        })
    }

    pub(crate) fn drain_controls(&self) -> impl Iterator<Item = PanelControl> + '_ {
        self.controls.try_iter()
    }

    pub(crate) fn publish(&mut self, values: &[(String, f32)]) {
        if self.last_publish.elapsed() < Duration::from_millis(100) {
            return;
        }
        self.last_publish = Instant::now();
        let values = values.iter().cloned().collect::<BTreeMap<_, _>>();
        let Ok(mut writer) = self.writer.lock() else {
            return;
        };
        let Some(stream) = writer.as_mut() else {
            return;
        };
        if serde_json::to_writer(&mut *stream, &ViewerToPanel::Snapshot { values: &values })
            .and_then(|_| stream.write_all(b"\n").map_err(serde_json::Error::io))
            .and_then(|_| stream.flush().map_err(serde_json::Error::io))
            .is_err()
        {
            *writer = None;
        }
    }
}

impl Drop for PanelBridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn accept_panel(
    listener: TcpListener,
    token: String,
    controls: mpsc::Sender<PanelControl>,
    writer: Arc<Mutex<Option<TcpStream>>>,
) {
    thread::spawn(move || {
        let Ok((stream, _address)) = listener.accept() else {
            return;
        };
        let Ok(reader_stream) = stream.try_clone() else {
            return;
        };
        let mut reader = BufReader::new(reader_stream);
        let mut hello = String::new();
        if reader.read_line(&mut hello).is_err() {
            return;
        }
        let Ok(PanelToViewer::Hello {
            token: received_token,
        }) = serde_json::from_str::<PanelToViewer>(&hello)
        else {
            return;
        };
        if received_token != token {
            return;
        }
        let _ = stream.set_nodelay(true);
        let Ok(writer_stream) = stream.try_clone() else {
            return;
        };
        if let Ok(mut destination) = writer.lock() {
            *destination = Some(writer_stream);
        }

        for line in reader.lines() {
            let Ok(line) = line else {
                break;
            };
            let Ok(PanelToViewer::Set { name, value }) =
                serde_json::from_str::<PanelToViewer>(&line)
            else {
                continue;
            };
            if name.is_empty() || name.len() >= 96 || !value.is_finite() {
                continue;
            }
            if controls.send(PanelControl { name, value }).is_err() {
                break;
            }
        }
        if let Ok(mut destination) = writer.lock() {
            *destination = None;
        }
    });
}

fn panel_executable() -> Result<PathBuf, RuntimeDiagnostic> {
    if let Some(path) = env::var_os("LOOM_UI_PANEL_BIN") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(panel_error(format!(
            "LOOM_UI_PANEL_BIN points to missing file `{}`",
            path.display()
        )));
    }
    let current = env::current_exe()
        .map_err(|error| panel_error(format!("could not locate Loom executable: {error}")))?;
    let sibling = current.with_file_name("loom-ui-panel");
    if sibling.is_file() {
        return Ok(sibling);
    }
    let resolved_sibling = current
        .canonicalize()
        .ok()
        .map(|path| path.with_file_name("loom-ui-panel"));
    if let Some(path) = resolved_sibling.as_ref().filter(|path| path.is_file()) {
        return Ok(path.clone());
    }
    Err(panel_error(format!(
        "project includes a UI, but neither `{}` nor its resolved-install sibling exists; \
         build or install the loom-ui-panel helper",
        sibling.display()
    )))
}

fn session_token() -> String {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let count = SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}-{:x}", std::process::id(), time, count)
}

fn panel_error(message: impl Into<String>) -> RuntimeDiagnostic {
    RuntimeDiagnostic::new(RuntimeDiagnosticCode::ProjectPanelFailed, message)
}
