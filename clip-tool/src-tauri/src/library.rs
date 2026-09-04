use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::SystemTime;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AudioTrackInfo {
    pub track_index: usize,
    pub process_name: String,
    #[serde(default)]
    pub wav_filename: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClipMetadataFile {
    pub duration_secs: f64,
    pub audio_tracks: Vec<AudioTrackInfo>,
    #[serde(default)]
    pub spike_markers: Vec<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClipItem {
    pub id: String,
    pub filename: String,
    pub full_path: String,
    pub game_tag: String,
    pub duration_secs: f64,
    pub created_at: String,
    pub audio_tracks: Vec<AudioTrackInfo>,
    #[serde(default)]
    pub spike_markers: Vec<f64>,
    pub preview_path: Option<String>,
    #[serde(default)]
    pub favorite: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiskSpaceInfo {
    pub total_gb: f64,
    pub free_gb: f64,
    pub used_gb: f64,
    pub is_low_space: bool,
}

#[tauri::command]
pub fn get_disk_space_info(app: AppHandle) -> DiskSpaceInfo {
    use tauri::Manager;
    let cfg = crate::config::get_config(app.clone());
    let clip_dir = if cfg.custom_clip_path.is_empty() {
        app.path().video_dir().unwrap_or_else(|_| PathBuf::from("C:\\"))
    } else {
        PathBuf::from(&cfg.custom_clip_path)
    };
    let check_dir = if clip_dir.exists() {
        clip_dir
    } else {
        clip_dir.ancestors().find(|p| p.exists()).map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("C:\\"))
    };

    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    use windows::core::HSTRING;

    let path_str = check_dir.to_string_lossy().to_string();
    let wide_path = HSTRING::from(path_str.as_str());

    let mut free_bytes_avail = 0u64;
    let mut total_bytes = 0u64;
    let mut total_free_bytes = 0u64;

    let ok = unsafe {
        GetDiskFreeSpaceExW(
            &wide_path,
            Some(&mut free_bytes_avail),
            Some(&mut total_bytes),
            Some(&mut total_free_bytes),
        ).is_ok()
    };

    if ok && total_bytes > 0 {
        let total_gb = total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let free_gb = free_bytes_avail as f64 / (1024.0 * 1024.0 * 1024.0);
        let used_gb = (total_bytes.saturating_sub(free_bytes_avail)) as f64 / (1024.0 * 1024.0 * 1024.0);
        let is_low_space = free_gb < 10.0;
        DiskSpaceInfo {
            total_gb: (total_gb * 10.0).round() / 10.0,
            free_gb: (free_gb * 10.0).round() / 10.0,
            used_gb: (used_gb * 10.0).round() / 10.0,
            is_low_space,
        }
    } else {
        DiskSpaceInfo {
            total_gb: 500.0,
            free_gb: 100.0,
            used_gb: 400.0,
            is_low_space: false,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Duration probing — ffprobe (precise JSON) with ffmpeg stderr fallback.
// ──────────────────────────────────────────────────────────────────────────────
pub async fn get_video_duration(app: &AppHandle, file_path: &str) -> f64 {
    if let Ok(cmd) = app.shell().sidecar("ffprobe") {
        let cmd = cmd.args(&[
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            file_path,
        ]);
        if let Ok(out) = cmd.output().await {
            if out.status.success() {
                if let Ok(s) = String::from_utf8(out.stdout) {
                    if let Ok(d) = s.trim().parse::<f64>() {
                        return d;
                    }
                }
            }
        }
    }

    // Fallback: scrape ffmpeg's stderr banner.
    if let Ok(cmd) = app.shell().sidecar("ffmpeg") {
        let cmd = cmd.args(&["-i", file_path]);
        if let Ok(out) = cmd.output().await {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if let Some(idx) = stderr.find("Duration: ") {
                let start = idx + 10;
                if start + 11 <= stderr.len() {
                    let parts: Vec<&str> = stderr[start..start + 11].split(':').collect();
                    if parts.len() == 3 {
                        let h: f64 = parts[0].parse().unwrap_or(0.0);
                        let m: f64 = parts[1].parse().unwrap_or(0.0);
                        let s: f64 = parts[2].parse().unwrap_or(0.0);
                        return h * 3600.0 + m * 60.0 + s;
                    }
                }
            }
        }
    }
    0.0
}

pub async fn generate_preview(app: &AppHandle, video_path: &str, duration: f64) -> Option<String> {
    let preview_buf = PathBuf::from(video_path).with_extension("jpg");
    let preview_str = preview_buf.to_string_lossy().to_string();

    if preview_buf.exists() {
        return Some(preview_str);
    }

    // Seek slightly after start: frame at exactly 0 can be black on some GPUs.
    let at = if duration > 2.0 { duration / 2.0 } else { 0.1 };
    if let Ok(cmd) = app.shell().sidecar("ffmpeg") {
        let cmd = cmd.args(&[
            "-y", "-ss", &format!("{at:.2}"),
            "-i", video_path,
            "-vframes", "1", "-q:v", "2",
            &preview_str,
        ]);
        if let Ok(out) = cmd.output().await {
            if out.status.success() {
                return Some(preview_str);
            }
        }
    }
    None
}

// ──────────────────────────────────────────────────────────────────────────────
// Favorites — persisted as a simple path list in the config dir.
// ──────────────────────────────────────────────────────────────────────────────
fn favorites_path(app: &AppHandle) -> PathBuf {
    let mut p = crate::config::get_config_path(app);
    p.set_file_name("favorites.json");
    p
}

fn load_favorites(app: &AppHandle) -> HashSet<String> {
    let path = favorites_path(app);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .map(|v| v.into_iter().collect())
        .unwrap_or_default()
}

fn save_favorites(app: &AppHandle, favs: &HashSet<String>) {
    let mut v: Vec<String> = favs.iter().cloned().collect();
    v.sort();
    let _ = std::fs::write(favorites_path(app), serde_json::to_string(&v).unwrap_or_default());
}

#[tauri::command]
pub fn toggle_favorite(app: AppHandle, path: String) -> Result<bool, String> {
    let mut favs = load_favorites(&app);
    let added = if favs.contains(&path) {
        favs.remove(&path);
        false
    } else {
        favs.insert(path);
        true
    };
    save_favorites(&app, &favs);
    Ok(added)
}

// ──────────────────────────────────────────────────────────────────────────────
// Clip listing — fast metadata scan, then bounded-parallel enrichment.
// ──────────────────────────────────────────────────────────────────────────────
struct PendingClip {
    path: PathBuf,
    filename: String,
    game_tag: String,
    created_at_str: String,
}

#[tauri::command]
pub async fn get_all_clips(app: AppHandle) -> Result<Vec<ClipItem>, String> {
    let config = crate::config::get_config(app.clone());
    let root = PathBuf::from(&config.custom_clip_path);

    let mut pending: Vec<PendingClip> = Vec::new();
    if !root.exists() {
        return Ok(vec![]);
    }

    if let Ok(mut game_entries) = tokio::fs::read_dir(&root).await {
        while let Ok(Some(game)) = game_entries.next_entry().await {
            let game_dir = game.path();
            if !game_dir.is_dir() { continue; }
            let game_tag = game_dir.file_name().unwrap_or_default().to_string_lossy().to_string();

            if let Ok(mut clip_entries) = tokio::fs::read_dir(&game_dir).await {
                while let Ok(Some(clip)) = clip_entries.next_entry().await {
                    let clip_path = clip.path();
                    if clip_path.extension().and_then(|s| s.to_str()) != Some("mp4") { continue; }

                    let filename = clip_path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if let Ok(metadata) = tokio::fs::metadata(&clip_path).await {
                        let created_at = metadata.modified().unwrap_or(SystemTime::now());
                        let datetime: chrono::DateTime<chrono::Local> = created_at.into();
                        pending.push(PendingClip {
                            path: clip_path,
                            filename,
                            game_tag: game_tag.clone(),
                            created_at_str: datetime.format("%Y-%m-%d %H:%M:%S").to_string(),
                        });
                    }
                }
            }
        }
    }

    // Enrich (sidecar metadata / duration probe / thumbnail) in parallel,
    // capped so we never stampede ffmpeg.
    let favorites = std::sync::Arc::new(load_favorites(&app));
    let sem = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
    let app = std::sync::Arc::new(app);

    let mut tasks = Vec::with_capacity(pending.len());
    for p in pending {
        let sem = sem.clone();
        let app = app.clone();
        let favorites = favorites.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.ok();
            enrich_clip(&app, p, &favorites).await
        }));
    }

    let mut clips = Vec::with_capacity(tasks.len());
    for t in tasks {
        if let Ok(Some(item)) = t.await {
            clips.push(item);
        }
    }

    clips.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(clips)
}

async fn enrich_clip(
    app: &std::sync::Arc<AppHandle>,
    p: PendingClip,
    favorites: &HashSet<String>,
) -> Option<ClipItem> {
    let clip_path = &p.path;

    let mut duration_secs = 0.0f64;
    let mut audio_tracks = vec![];
    let mut spike_markers = vec![];

    let tracks_json = clip_path.with_file_name(format!(
        "{}_tracks.json",
        clip_path.file_stem().unwrap_or_default().to_string_lossy()
    ));
    if tracks_json.exists() {
        if let Ok(json) = tokio::fs::read_to_string(&tracks_json).await {
            if let Ok(meta) = serde_json::from_str::<ClipMetadataFile>(&json) {
                duration_secs = meta.duration_secs;
                audio_tracks = meta.audio_tracks;
                spike_markers = meta.spike_markers;
            } else if let Ok(tracks) = serde_json::from_str::<Vec<AudioTrackInfo>>(&json) {
                audio_tracks = tracks; // legacy format
            }
        }
    }

    if duration_secs <= 0.0 {
        duration_secs = get_video_duration(app, clip_path.to_str().unwrap_or("")).await;
    }

    let preview_path = generate_preview(app, clip_path.to_str().unwrap_or(""), duration_secs).await;

    Some(ClipItem {
        id: p.filename.clone(),
        filename: p.filename,
        full_path: clip_path.to_string_lossy().to_string(),
        game_tag: p.game_tag,
        duration_secs,
        created_at: p.created_at_str,
        audio_tracks,
        spike_markers,
        preview_path,
        favorite: favorites.contains(clip_path.to_string_lossy().as_ref()),
    })
}

/// Move a clip AND its sidecars (WAVs, _tracks.json, preview jpg) to the
/// Recycle Bin. The path must live inside the configured clips folder.
#[tauri::command]
pub fn delete_clip(app: AppHandle, path: String) -> Result<(), String> {
    let config = crate::config::get_config(app.clone());
    let root = std::fs::canonicalize(&config.custom_clip_path)
        .map_err(|e| format!("Clips folder unavailable: {e}"))?;
    let target = std::fs::canonicalize(&path)
        .map_err(|_| "Clip no longer exists".to_string())?;

    if !target.starts_with(&root) {
        return Err("Refusing to delete outside the clips folder".into());
    }
    if target.extension().and_then(|s| s.to_str()) != Some("mp4") {
        return Err("Not a clip file".into());
    }

    // Collect sidecars before removing the mp4 name.
    // Strictly match only direct sidecars of THIS clip and avoid touching edited versions (e.g. _edited.mp4, _edited.jpg, _edited_tracks.json)
    let stem = target.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let mut victims: Vec<PathBuf> = vec![target.clone()];
    if let Some(dir) = target.parent() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.filter_map(Result::ok) {
                let p = e.path();
                let fname = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
                if p == target {
                    continue;
                }

                let is_direct_preview = ext == "jpg" && fname == format!("{stem}.jpg");
                let is_direct_meta = ext == "json" && fname == format!("{stem}_tracks.json");
                let is_direct_wav = ext == "wav"
                    && fname.starts_with(&format!("{stem}_"))
                    && !fname.starts_with(&format!("{stem}_edited"));

                if is_direct_preview || is_direct_meta || is_direct_wav {
                    victims.push(p);
                }
            }
        }
    }

    let mut first_err: Option<String> = None;
    for v in &victims {
        let clean_str = v.to_string_lossy();
        let clean_v = PathBuf::from(clean_str.trim_start_matches(r"\\?\"));
        if let Err(e) = trash::delete(&clean_v) {
            if first_err.is_none() {
                first_err = Some(e.to_string());
            }
        }
    }

    if let Some(err) = first_err {
        return Err(format!("Recycle Bin failed: {err}"));
    }

    if let Some(parent) = target.parent() {
        if parent != root {
            if let Ok(mut entries) = std::fs::read_dir(parent) {
                if entries.next().is_none() {
                    let _ = std::fs::remove_dir(parent);
                }
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub fn copy_clip_to_clipboard(path: String) -> Result<(), String> {
    use windows::Win32::System::DataExchange::{OpenClipboard, EmptyClipboard, SetClipboardData, CloseClipboard};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GHND};
    use windows::Win32::Foundation::GlobalFree;
    use windows::Win32::UI::Shell::DROPFILES;

    let path_buf = PathBuf::from(&path);
    if !path_buf.exists() {
        return Err("File does not exist".into());
    }
    let abs_path = path_buf.canonicalize().map_err(|e| e.to_string())?;
    let path_str = abs_path.to_string_lossy();
    let clean_path = path_str.trim_start_matches(r"\\?\");

    let mut wide_path: Vec<u16> = clean_path.encode_utf16().collect();
    wide_path.push(0);
    wide_path.push(0); // Double null-terminated

    let dropfiles_size = std::mem::size_of::<DROPFILES>();
    let total_size = dropfiles_size + wide_path.len() * 2;

    unsafe {
        let h_global = GlobalAlloc(GHND, total_size).map_err(|e| format!("GlobalAlloc failed: {e}"))?;
        let ptr = GlobalLock(h_global);
        if ptr.is_null() {
            return Err("GlobalLock failed".into());
        }

        let dropfiles = ptr as *mut DROPFILES;
        (*dropfiles).pFiles = dropfiles_size as u32;
        (*dropfiles).fWide = windows::core::BOOL(1); // Unicode UTF-16

        let dest = (ptr as usize + dropfiles_size) as *mut u16;
        std::ptr::copy_nonoverlapping(wide_path.as_ptr(), dest, wide_path.len());

        let _ = GlobalUnlock(h_global);

        if OpenClipboard(None).is_err() {
            let _ = GlobalFree(Some(h_global));
            return Err("OpenClipboard failed".into());
        }
        let _ = EmptyClipboard();
        const CF_HDROP: u32 = 15;
        let res = SetClipboardData(CF_HDROP, Some(windows::Win32::Foundation::HANDLE(h_global.0)));
        let _ = CloseClipboard();

        if res.is_err() {
            let _ = GlobalFree(Some(h_global));
            return Err("SetClipboardData failed".into());
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CleanupSummary {
    pub clips_deleted: usize,
    pub space_freed_mb: f64,
}

#[tauri::command]
pub fn trigger_auto_cleanup(app: AppHandle) -> Result<CleanupSummary, String> {
    run_auto_cleanup(&app)
}

pub fn run_auto_cleanup(app: &AppHandle) -> Result<CleanupSummary, String> {
    let cfg = crate::config::get_config(app.clone());
    if !cfg.cleanup.enabled {
        return Ok(CleanupSummary { clips_deleted: 0, space_freed_mb: 0.0 });
    }

    let root = PathBuf::from(&cfg.custom_clip_path);
    if !root.exists() {
        return Ok(CleanupSummary { clips_deleted: 0, space_freed_mb: 0.0 });
    }

    let favorites = load_favorites(app);
    let now = SystemTime::now();

    struct CleanupCandidate {
        path: PathBuf,
        created: SystemTime,
        size_bytes: u64,
    }

    let mut candidates: Vec<CleanupCandidate> = Vec::new();
    let mut total_clips_size_bytes: u64 = 0;

    if let Ok(game_entries) = std::fs::read_dir(&root) {
        for game in game_entries.filter_map(Result::ok) {
            let game_dir = game.path();
            if !game_dir.is_dir() { continue; }
            if let Ok(clip_entries) = std::fs::read_dir(&game_dir) {
                for clip in clip_entries.filter_map(Result::ok) {
                    let clip_path = clip.path();
                    if clip_path.extension().and_then(|s| s.to_str()) != Some("mp4") { continue; }

                    let path_str = clip_path.to_string_lossy().to_string();
                    let is_fav = favorites.contains(&path_str);

                    let meta = match clip_path.metadata() {
                        Ok(m) => m,
                        Err(_) => continue,
                    };
                    let size = meta.len();
                    total_clips_size_bytes += size;

                    if !is_fav {
                        let created = meta.modified().unwrap_or(now);
                        candidates.push(CleanupCandidate {
                            path: clip_path,
                            created,
                            size_bytes: size,
                        });
                    }
                }
            }
        }
    }

    candidates.sort_by(|a, b| a.created.cmp(&b.created));

    let mut deleted_count = 0;
    let mut freed_bytes = 0u64;

    // 1. Age-based cleanup
    if cfg.cleanup.max_age_days > 0 {
        let max_age_secs = (cfg.cleanup.max_age_days as u64) * 24 * 3600;
        let mut remaining = Vec::new();
        for c in candidates {
            let age_secs = now.duration_since(c.created).unwrap_or_default().as_secs();
            if age_secs > max_age_secs {
                if let Ok(_) = delete_clip(app.clone(), c.path.to_string_lossy().to_string()) {
                    deleted_count += 1;
                    freed_bytes += c.size_bytes;
                    total_clips_size_bytes = total_clips_size_bytes.saturating_sub(c.size_bytes);
                }
            } else {
                remaining.push(c);
            }
        }
        candidates = remaining;
    }

    // 2. Storage size limit cleanup
    if cfg.cleanup.max_storage_gb > 0.0 {
        let max_bytes = (cfg.cleanup.max_storage_gb * 1024.0 * 1024.0 * 1024.0) as u64;
        let mut remaining = Vec::new();
        for c in candidates {
            if total_clips_size_bytes > max_bytes {
                if let Ok(_) = delete_clip(app.clone(), c.path.to_string_lossy().to_string()) {
                    deleted_count += 1;
                    freed_bytes += c.size_bytes;
                    total_clips_size_bytes = total_clips_size_bytes.saturating_sub(c.size_bytes);
                }
            } else {
                remaining.push(c);
            }
        }
        candidates = remaining;
    }

    // 3. Minimum free disk space cleanup
    if cfg.cleanup.min_free_disk_gb > 0.0 {
        let disk_info = get_disk_space_info(app.clone());
        let mut current_free_gb = disk_info.free_gb;
        if current_free_gb < cfg.cleanup.min_free_disk_gb {
            for c in candidates {
                if current_free_gb >= cfg.cleanup.min_free_disk_gb {
                    break;
                }
                if let Ok(_) = delete_clip(app.clone(), c.path.to_string_lossy().to_string()) {
                    deleted_count += 1;
                    freed_bytes += c.size_bytes;
                    current_free_gb += c.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                }
            }
        }
    }

    let space_freed_mb = (freed_bytes as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0;
    if deleted_count > 0 {
        println!("[auto_cleanup] deleted {} old clips, freed {:.1} MB", deleted_count, space_freed_mb);
    }

    Ok(CleanupSummary {
        clips_deleted: deleted_count,
        space_freed_mb,
    })
}