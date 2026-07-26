use std::{
    collections::BTreeMap,
    env, fs,
    io::{BufRead, BufReader, Write},
    net::TcpStream,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};

use serde::{Deserialize, Serialize};
use tauri::{WebviewUrl, WebviewWindowBuilder, http};

#[derive(Clone, Debug)]
struct PanelArguments {
    address: String,
    token: String,
    root: PathBuf,
    entry: String,
    title: String,
    width: f64,
    height: f64,
}

#[derive(Clone)]
struct PanelState {
    writer: Arc<Mutex<TcpStream>>,
    snapshot: Arc<Mutex<PanelSnapshot>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PanelSnapshot {
    connected: bool,
    values: BTreeMap<String, f32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PanelToViewer {
    Hello { token: String },
    Set { name: String, value: f32 },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ViewerToPanel {
    Snapshot { values: BTreeMap<String, f32> },
}

#[tauri::command]
fn set_control(
    state: tauri::State<'_, PanelState>,
    name: String,
    value: f32,
) -> Result<(), String> {
    if name.is_empty() || name.len() >= 96 || !value.is_finite() {
        return Err("invalid panel control".to_owned());
    }
    send_message(&state.writer, &PanelToViewer::Set { name, value })
}

#[tauri::command]
fn get_snapshot(state: tauri::State<'_, PanelState>) -> Result<PanelSnapshot, String> {
    state
        .snapshot
        .lock()
        .map(|snapshot| snapshot.clone())
        .map_err(|_| "panel snapshot lock was poisoned".to_owned())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("loom UI panel: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = parse_arguments(env::args().skip(1))?;
    let mut stream = TcpStream::connect(&arguments.address)
        .map_err(|error| format!("could not connect to viewer: {error}"))?;
    stream
        .set_nodelay(true)
        .map_err(|error| format!("could not configure viewer connection: {error}"))?;
    write_line(
        &mut stream,
        &PanelToViewer::Hello {
            token: arguments.token.clone(),
        },
    )?;

    let reader = stream
        .try_clone()
        .map_err(|error| format!("could not clone viewer connection: {error}"))?;
    let snapshot = Arc::new(Mutex::new(PanelSnapshot {
        connected: true,
        values: BTreeMap::new(),
    }));
    read_snapshots(reader, snapshot.clone());

    let state = PanelState {
        writer: Arc::new(Mutex::new(stream)),
        snapshot,
    };
    let asset_root = arguments.root.clone();
    let asset_entry = arguments.entry.clone();
    let window_arguments = arguments.clone();

    tauri::Builder::default()
        .manage(state)
        .register_uri_scheme_protocol("loom-ui", move |_context, request| {
            serve_asset(&asset_root, &asset_entry, request.uri().path())
        })
        .invoke_handler(tauri::generate_handler![set_control, get_snapshot])
        .setup(move |app| {
            let url = tauri::Url::parse("loom-ui://localhost/")
                .map_err(|error| format!("invalid panel URL: {error}"))?;
            WebviewWindowBuilder::new(app, "loom-project-panel", WebviewUrl::CustomProtocol(url))
                .title(&window_arguments.title)
                .inner_size(window_arguments.width, window_arguments.height)
                .min_inner_size(320.0, 520.0)
                .resizable(true)
                .focused(true)
                .always_on_top(true)
                .visible_on_all_workspaces(true)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .map_err(|error| error.to_string())
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<PanelArguments, String> {
    let mut address = None;
    let mut token = None;
    let mut root = None;
    let mut entry = None;
    let mut title = None;
    let mut width = None;
    let mut height = None;
    let mut arguments = arguments;
    while let Some(flag) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for `{flag}`"))?;
        match flag.as_str() {
            "--address" => address = Some(value),
            "--token" => token = Some(value),
            "--root" => root = Some(PathBuf::from(value)),
            "--entry" => entry = Some(value),
            "--title" => title = Some(value),
            "--width" => width = Some(parse_dimension(&value, &flag)?),
            "--height" => height = Some(parse_dimension(&value, &flag)?),
            _ => return Err(format!("unknown argument `{flag}`")),
        }
    }
    let root = root.ok_or("missing --root")?;
    if !root.is_dir() {
        return Err(format!(
            "panel asset root `{}` does not exist",
            root.display()
        ));
    }
    let entry = entry.unwrap_or_else(|| "index.html".to_owned());
    validate_relative_path(&entry)?;
    Ok(PanelArguments {
        address: address.ok_or("missing --address")?,
        token: token.ok_or("missing --token")?,
        root,
        entry,
        title: title.unwrap_or_else(|| "Loom Controls".to_owned()),
        width: width.unwrap_or(380.0),
        height: height.unwrap_or(720.0),
    })
}

fn parse_dimension(value: &str, flag: &str) -> Result<f64, String> {
    let value = value
        .parse::<f64>()
        .map_err(|_| format!("`{flag}` must be a number"))?;
    if !value.is_finite() || value < 200.0 || value > 4096.0 {
        return Err(format!("`{flag}` must be between 200 and 4096"));
    }
    Ok(value)
}

fn validate_relative_path(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err(format!("panel path `{}` is not relative", path.display()));
    }
    Ok(())
}

fn serve_asset(root: &Path, entry: &str, request_path: &str) -> http::Response<Vec<u8>> {
    let relative = request_path.trim_start_matches('/');
    let relative = if relative.is_empty() { entry } else { relative };
    if validate_relative_path(relative).is_err() {
        return response(
            http::StatusCode::BAD_REQUEST,
            "text/plain",
            b"invalid path".to_vec(),
        );
    }
    let path = root.join(relative);
    match fs::read(&path) {
        Ok(bytes) => response(http::StatusCode::OK, content_type(&path), bytes),
        Err(_) => response(
            http::StatusCode::NOT_FOUND,
            "text/plain",
            b"panel asset not found".to_vec(),
        ),
    }
}

fn response(
    status: http::StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, content_type)
        .header(http::header::CACHE_CONTROL, "no-store")
        .body(body)
        .expect("static panel response is valid")
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn read_snapshots(stream: TcpStream, snapshot: Arc<Mutex<PanelSnapshot>>) {
    thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            let Ok(line) = line else {
                break;
            };
            let Ok(ViewerToPanel::Snapshot { values }) =
                serde_json::from_str::<ViewerToPanel>(&line)
            else {
                continue;
            };
            if let Ok(mut snapshot) = snapshot.lock() {
                snapshot.connected = true;
                snapshot.values = values;
            }
        }
        if let Ok(mut snapshot) = snapshot.lock() {
            snapshot.connected = false;
        }
    });
}

fn send_message(writer: &Mutex<TcpStream>, message: &PanelToViewer) -> Result<(), String> {
    let mut writer = writer
        .lock()
        .map_err(|_| "viewer connection lock was poisoned".to_owned())?;
    write_line(&mut writer, message)
}

fn write_line(stream: &mut TcpStream, message: &PanelToViewer) -> Result<(), String> {
    serde_json::to_writer(&mut *stream, message).map_err(|error| error.to_string())?;
    stream.write_all(b"\n").map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())
}
