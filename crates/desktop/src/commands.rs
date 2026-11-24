//! Commands the web UI can call. All text returned to the UI is either data from the game files
//! or an error code; the UI owns every user-facing sentence (and its translations).

use adv_core::browse::FileKind;
use adv_core::preview::{AudioStream, Options, Preview};
use adv_session::{Item, Session, SessionError};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::State;
use tauri::ipc::Response;

/// The session the window currently looks at.
#[derive(Default)]
pub struct AppState {
    session: Mutex<Option<Arc<Session>>>,
}

/// An error for the UI: a stable code to translate plus the technical English detail.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct AppError {
    /// One of `bad_path`, `not_found`, `wrong_kind`, `failed`, `no_session`.
    pub code: &'static str,
    /// Technical message from the core, shown in a details line.
    pub detail: String,
}

impl From<SessionError> for AppError {
    fn from(e: SessionError) -> Self {
        let code = match &e {
            SessionError::BadPath(_) => "bad_path",
            SessionError::NotFound(_) => "not_found",
            SessionError::WrongKind(_) => "wrong_kind",
            SessionError::Failed(_) => "failed",
        };
        Self {
            code,
            detail: e.to_string(),
        }
    }
}

impl AppState {
    fn session(&self) -> Result<Arc<Session>, AppError> {
        self.session
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .ok_or(AppError {
                code: "no_session",
                detail: "no folder is open".into(),
            })
    }
}

/// A directory row for the UI.
#[derive(Debug, Serialize, PartialEq)]
pub struct ItemDto {
    name: String,
    is_dir: bool,
    is_pak: bool,
    size: Option<u64>,
    files: Option<usize>,
    kind: &'static str,
}

impl From<Item> for ItemDto {
    fn from(i: Item) -> Self {
        Self {
            name: i.name,
            is_dir: i.is_dir,
            is_pak: i.is_pak,
            size: i.size,
            files: i.files,
            kind: i.kind.name(),
        }
    }
}

/// A sound stream for the UI.
#[derive(Debug, Serialize, PartialEq)]
pub struct StreamDto {
    name: String,
    frequency: Option<u32>,
    channels: u32,
    seconds: Option<f64>,
}

impl From<&AudioStream> for StreamDto {
    fn from(s: &AudioStream) -> Self {
        Self {
            name: s.name.clone(),
            frequency: s.frequency,
            channels: s.channels,
            seconds: s.seconds,
        }
    }
}

/// What the video player needs to size and convert the frames of `video_frames`.
#[derive(Debug, Serialize, PartialEq)]
pub struct VideoInfoDto {
    pub width: u32,
    pub height: u32,
    pub chroma_shift_x: u32,
    pub chroma_shift_y: u32,
    pub fps: f64,
    pub frames: u64,
    /// Size of one frame in bytes (the three planes one after another).
    pub frame_bytes: usize,
}

/// A preview for the UI. Picture and sound bytes come from separate binary commands.
#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PreviewDto {
    /// Text.
    Text { text: String, truncated: bool },
    /// A picture; fetch the PNG with `get_image`.
    Image {
        width: u32,
        height: u32,
        format: String,
        guessed: bool,
        candidates: Vec<(u32, u32)>,
        index: usize,
        labels: Vec<String>,
    },
    /// A 3D model; fetch the buffers with `get_mesh`.
    Model {
        vertices: usize,
        triangles: usize,
        stride: usize,
    },
    /// A skeletal animation clip (header only).
    Animation {
        fps: u16,
        frames: u16,
        seconds: f64,
        bones: u32,
    },
    /// A Theora video (header only).
    Video {
        width: u32,
        height: u32,
        fps: f64,
        seconds: Option<f64>,
        frames: Option<u64>,
    },
    /// A sound bank; fetch a stream with `get_audio`.
    Audio { streams: Vec<StreamDto> },
    /// A `.bev` FMOD Designer project.
    Bev {
        version: u32,
        project: String,
        languages: Vec<String>,
        chunks: Vec<(String, u32)>,
        names: Vec<String>,
        paths: Vec<String>,
    },
    /// An SVG picture, already sanitised by the core; `source` is the original text.
    Svg {
        svg: String,
        source: String,
        removed: usize,
        removed_names: Vec<String>,
    },
    /// A `.loc` localization file; read its items with `loc_page`.
    Loc { count: usize, keys: bool },
    /// Raw bytes.
    Binary { len: usize, hex: String },
}

impl From<Preview> for PreviewDto {
    fn from(p: Preview) -> Self {
        match p {
            Preview::Text { text, truncated } => Self::Text { text, truncated },
            Preview::Bev {
                version,
                project,
                languages,
                chunks,
                names,
                paths,
            } => Self::Bev {
                version,
                project,
                languages,
                chunks,
                names,
                paths,
            },
            Preview::Svg {
                svg,
                source,
                removed,
                removed_names,
            } => Self::Svg {
                svg,
                source,
                removed,
                removed_names,
            },
            Preview::Loc { count, keys } => Self::Loc { count, keys },
            Preview::Image {
                width,
                height,
                format,
                guessed,
                candidates,
                index,
                labels,
                ..
            } => Self::Image {
                width,
                height,
                format,
                guessed,
                candidates,
                index,
                labels,
            },
            Preview::Model {
                vertices,
                triangles,
                stride,
            } => Self::Model {
                vertices,
                triangles,
                stride,
            },
            Preview::Animation {
                fps,
                frames,
                seconds,
                bones,
            } => Self::Animation {
                fps,
                frames,
                seconds,
                bones,
            },
            Preview::Video {
                width,
                height,
                fps,
                seconds,
                frames,
            } => Self::Video {
                width,
                height,
                fps,
                seconds,
                frames,
            },
            Preview::Audio { streams } => Self::Audio {
                streams: streams.iter().map(StreamDto::from).collect(),
            },
            Preview::Binary { len, hex } => Self::Binary { len, hex },
        }
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError {
            code: "failed",
            detail: e.to_string(),
        })?
}

/// Opens a data folder or one `.pak`; returns the folder shown as the root.
#[tauri::command]
async fn open_path(state: State<'_, AppState>, path: String) -> Result<String, AppError> {
    let session =
        blocking(move || Session::open(&PathBuf::from(path)).map_err(AppError::from)).await?;
    let root = session.root().display().to_string();
    if let Ok(mut guard) = state.session.lock() {
        *guard = Some(Arc::new(session));
    }
    Ok(root)
}

/// Lists a directory of the open session.
#[tauri::command]
async fn list_dir(state: State<'_, AppState>, path: String) -> Result<Vec<ItemDto>, AppError> {
    let s = state.session()?;
    blocking(move || {
        s.list(&path)
            .map(|v| v.into_iter().map(ItemDto::from).collect())
            .map_err(AppError::from)
    })
    .await
}

/// Real kinds of the given files (a `.bin` can be a texture or a model).
#[tauri::command]
async fn file_kinds(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<Vec<(String, &'static str)>, AppError> {
    let s = state.session()?;
    blocking(move || {
        Ok(s.kinds(&paths)
            .into_iter()
            .map(|(p, k)| (p, k.name()))
            .collect())
    })
    .await
}

/// Totals for the open data: file, archive and byte counts, and per kind.
#[derive(Debug, Serialize, PartialEq)]
pub struct CensusDto {
    files: u64,
    paks: u64,
    bytes: u64,
    kinds: Vec<KindCount>,
}

/// One match of a search inside texts.
#[derive(Debug, Serialize, PartialEq)]
pub struct TextHitDto {
    path: String,
    /// Item number inside a `.loc` file.
    id: Option<u32>,
    snippet: String,
}

/// One `.loc` item.
#[derive(Debug, Serialize, PartialEq)]
pub struct LocItemDto {
    id: u32,
    key: Option<String>,
    text: String,
}

/// A page of `.loc` items.
#[derive(Debug, Serialize, PartialEq)]
pub struct LocPageDto {
    total: usize,
    matched: usize,
    offset: usize,
    items: Vec<LocItemDto>,
}

/// Files of one kind and their total size.
#[derive(Debug, Serialize, PartialEq)]
pub struct KindCount {
    kind: &'static str,
    count: u64,
    bytes: u64,
}

/// Game version read from `Profiles/game.version`, if the folder has it.
#[derive(Debug, Serialize, PartialEq)]
pub struct GameVersionDto {
    head: String,
    version: String,
    /// Source-control revision from a `.rev` file, empty when there is none.
    revision: String,
    /// Date of that revision (`YYYY-MM-DD`), empty when there is none.
    date: String,
}

/// The version of the opened game data, or nothing for raw data without that file.
#[tauri::command]
async fn game_version(state: State<'_, AppState>) -> Result<Option<GameVersionDto>, AppError> {
    let s = state.session()?;
    blocking(move || {
        let v = s.game_version();
        let b = s.build_info().unwrap_or_default();
        if v.is_none() && b.version.is_empty() {
            return Ok(None);
        }
        let (head, version) = v.map_or((String::new(), b.version), |v| (v.head, v.version));
        Ok(Some(GameVersionDto {
            head,
            version,
            revision: b.revision,
            date: b.date,
        }))
    })
    .await
}

/// Counts everything in the open data (by file name, so a plain `.bin` counts as "other").
#[tauri::command]
async fn census(state: State<'_, AppState>) -> Result<CensusDto, AppError> {
    let s = state.session()?;
    blocking(move || {
        let c = s.census().map_err(AppError::from)?;
        Ok(CensusDto {
            files: c.files,
            paks: c.paks,
            bytes: c.bytes,
            kinds: c
                .kinds
                .into_iter()
                .map(|(k, count, bytes)| KindCount {
                    kind: k.name(),
                    count,
                    bytes,
                })
                .collect(),
        })
    })
    .await
}

/// Completions for the path box (folders and `.pak` files); needs no open session.
#[tauri::command]
async fn complete_path(prefix: String) -> Result<Vec<String>, AppError> {
    blocking(move || Ok(adv_session::complete_path(&prefix))).await
}

/// Lists files of one kind anywhere in the open data (at most 1000).
/// Most files one kind list returns (the page shows them in pages).
const KIND_LIST_LIMIT: usize = 200_000;

#[tauri::command]
async fn find_kind(state: State<'_, AppState>, kind: String) -> Result<Vec<String>, AppError> {
    let s = state.session()?;
    let kind = FileKind::from_name(&kind).ok_or(AppError {
        code: "failed",
        detail: format!("unknown kind {kind}"),
    })?;
    blocking(move || s.find_kind(kind, KIND_LIST_LIMIT).map_err(AppError::from)).await
}

/// Finds files by part of the name (at most 300 hits).
#[tauri::command]
async fn search(
    state: State<'_, AppState>,
    query: String,
    scope: Option<String>,
) -> Result<Vec<String>, AppError> {
    let s = state.session()?;
    let scope = scope.unwrap_or_default();
    blocking(move || s.search_in(&scope, &query, 300).map_err(AppError::from)).await
}

/// Finds files by part of their text, including `.loc` keys and texts (at most 300 hits).
#[tauri::command]
async fn search_text(
    state: State<'_, AppState>,
    query: String,
    scope: Option<String>,
) -> Result<Vec<TextHitDto>, AppError> {
    let s = state.session()?;
    let scope = scope.unwrap_or_default();
    blocking(move || {
        s.search_text_in(&scope, &query, 300)
            .map(|v| {
                v.into_iter()
                    .map(|h| TextHitDto {
                        path: h.path,
                        id: h.id,
                        snippet: h.snippet,
                    })
                    .collect()
            })
            .map_err(AppError::from)
    })
    .await
}

/// One page of the items of a `.loc` file, optionally filtered by a query.
#[tauri::command]
async fn loc_page(
    state: State<'_, AppState>,
    path: String,
    query: String,
    offset: usize,
    limit: usize,
) -> Result<LocPageDto, AppError> {
    let s = state.session()?;
    blocking(move || {
        s.loc_page(&path, &query, offset, limit)
            .map(|p| LocPageDto {
                total: p.total,
                matched: p.matched,
                offset: p.offset,
                items: p
                    .items
                    .into_iter()
                    .map(|i| LocItemDto {
                        id: i.id,
                        key: i.key,
                        text: i.text,
                    })
                    .collect(),
            })
            .map_err(AppError::from)
    })
    .await
}

/// Describes a file for the preview pane.
#[tauri::command]
async fn preview(
    state: State<'_, AppState>,
    path: String,
    texture_index: Option<usize>,
) -> Result<PreviewDto, AppError> {
    let s = state.session()?;
    blocking(move || {
        s.preview(&path, Options { texture_index })
            .map(PreviewDto::from)
            .map_err(AppError::from)
    })
    .await
}

/// The PNG of an image preview.
#[tauri::command]
async fn get_image(
    state: State<'_, AppState>,
    path: String,
    texture_index: Option<usize>,
) -> Result<Response, AppError> {
    let s = state.session()?;
    let png = blocking(move || match s.preview(&path, Options { texture_index }) {
        Ok(Preview::Image { png, .. }) => Ok(png),
        Ok(_) => Err(AppError {
            code: "wrong_kind",
            detail: format!("{path}: not a picture"),
        }),
        Err(e) => Err(AppError::from(e)),
    })
    .await?;
    Ok(Response::new(png))
}

/// A model packed as described in `adv_core::preview::mesh_bytes`.
#[tauri::command]
async fn get_mesh(state: State<'_, AppState>, path: String) -> Result<Response, AppError> {
    let s = state.session()?;
    let bytes = blocking(move || s.mesh(&path).map_err(AppError::from)).await?;
    Ok(Response::new(bytes))
}

/// Animation clips next to a model (virtual paths).
#[tauri::command]
async fn get_clips(state: State<'_, AppState>, path: String) -> Result<Vec<String>, AppError> {
    let s = state.session()?;
    blocking(move || s.clips_of(&path).map_err(AppError::from)).await
}

/// A model with one animation clip as a binary glTF file.
#[tauri::command]
async fn get_animated_glb(
    state: State<'_, AppState>,
    path: String,
    clip: String,
) -> Result<Response, AppError> {
    let s = state.session()?;
    let bytes = blocking(move || s.glb_animated(&path, &clip).map_err(AppError::from)).await?;
    Ok(Response::new(bytes))
}

/// A model with the textures of its elements (and optionally one clip) as a binary glTF file.
#[tauri::command]
async fn get_textured_glb(
    state: State<'_, AppState>,
    path: String,
    clip: Option<String>,
) -> Result<Response, AppError> {
    let s = state.session()?;
    let bytes = blocking(move || {
        s.glb_textured(&path, clip.as_deref())
            .map_err(AppError::from)
    })
    .await?;
    Ok(Response::new(bytes))
}

/// One decoded sound stream as a WAV file.
#[tauri::command]
async fn get_audio(
    state: State<'_, AppState>,
    path: String,
    index: usize,
) -> Result<Response, AppError> {
    let s = state.session()?;
    let wav = blocking(move || s.audio_wav(&path, index).map_err(AppError::from)).await?;
    Ok(Response::new(wav))
}

/// Opens a video for the player.
#[tauri::command]
async fn video_open(state: State<'_, AppState>, path: String) -> Result<VideoInfoDto, AppError> {
    let s = state.session()?;
    let m = blocking(move || s.video_open(&path).map_err(AppError::from)).await?;
    Ok(VideoInfoDto {
        width: m.width,
        height: m.height,
        chroma_shift_x: m.chroma_shift_x,
        chroma_shift_y: m.chroma_shift_y,
        fps: f64::from(m.fps_num) / f64::from(m.fps_den.max(1)),
        frames: m.frames,
        frame_bytes: m.frame_bytes(),
    })
}

/// The soundtrack of a video as WAV (empty when it has none).
#[tauri::command]
async fn video_audio(state: State<'_, AppState>, path: String) -> Result<Response, AppError> {
    let s = state.session()?;
    let wav = blocking(move || s.video_audio(&path).map_err(AppError::from)).await?;
    Ok(Response::new(wav))
}

/// Up to `count` decoded frames of a video starting at `start`, as planar YCbCr.
#[tauri::command]
async fn video_frames(
    state: State<'_, AppState>,
    path: String,
    start: u64,
    count: usize,
) -> Result<Response, AppError> {
    let s = state.session()?;
    let bytes =
        blocking(move || s.video_frames(&path, start, count).map_err(AppError::from)).await?;
    Ok(Response::new(bytes))
}

/// Writes a file or its converted form to a place the user chose in the save dialog.
///
/// `format` is `raw` (the file as stored), `png` (a picture), `wav` (one sound stream, chosen
/// by `index`) `obj` or `glb` (a model), `txt`, `html` or `xml` (a text file or `.loc`).
#[tauri::command]
async fn save_as(
    state: State<'_, AppState>,
    path: String,
    format: String,
    dest: String,
    texture_index: Option<usize>,
    index: Option<usize>,
) -> Result<(), AppError> {
    let s = state.session()?;
    blocking(move || {
        let target = PathBuf::from(&dest);
        if !target.is_absolute() {
            return Err(AppError {
                code: "bad_path",
                detail: format!("{dest}: the destination must be an absolute path"),
            });
        }
        let bytes: Vec<u8> = match format.as_str() {
            "raw" => s.read(&path)?,
            "png" => match s.preview(&path, Options { texture_index })? {
                Preview::Image { png, .. } => png,
                _ => {
                    return Err(AppError {
                        code: "wrong_kind",
                        detail: format!("{path}: not a picture"),
                    });
                }
            },
            "wav" => s.audio_wav(&path, index.unwrap_or(0))?,
            "mp3" => s.audio_mp3(&path, index.unwrap_or(0))?,
            "obj" => s.obj(&path)?.into_bytes(),
            "glb" => s.glb(&path)?,
            "txt" | "html" | "xml" => s.export_text(&path, &format)?,
            other => {
                return Err(AppError {
                    code: "wrong_kind",
                    detail: format!("unknown export format {other:?}"),
                });
            }
        };
        std::fs::write(&target, bytes).map_err(|e| AppError {
            code: "failed",
            detail: format!("{}: {e}", target.display()),
        })
    })
    .await
}

/// Starts the app.
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            open_path,
            list_dir,
            file_kinds,
            search,
            search_text,
            loc_page,
            preview,
            get_image,
            get_mesh,
            get_clips,
            get_animated_glb,
            get_textured_glb,
            get_audio,
            video_open,
            video_frames,
            video_audio,
            save_as,
            census,
            find_kind,
            game_version,
            complete_path
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("the app failed to start: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_get_stable_codes() {
        let e = AppError::from(SessionError::NotFound("x".into()));
        assert_eq!(e.code, "not_found");
        assert!(e.detail.contains('x'));
        assert_eq!(
            AppError::from(SessionError::BadPath("..".into())).code,
            "bad_path"
        );
        assert_eq!(
            AppError::from(SessionError::WrongKind("d".into())).code,
            "wrong_kind"
        );
        assert_eq!(
            AppError::from(SessionError::Failed("f".into())).code,
            "failed"
        );
        let none = AppState::default().session().err().map(|e| e.code);
        assert_eq!(none, Some("no_session"));
    }

    #[test]
    fn preview_json_has_a_kind_tag_and_no_picture_bytes() {
        let dto = PreviewDto::from(Preview::Image {
            width: 4,
            height: 4,
            png: vec![1, 2, 3],
            format: "DXT1".into(),
            guessed: true,
            candidates: vec![(4, 4)],
            index: 0,
            labels: vec!["DXT1".into()],
        });
        let json = format!("{dto:?}");
        assert!(!json.contains("png"));
        assert!(matches!(dto, PreviewDto::Image { width: 4, .. }));
    }
}
