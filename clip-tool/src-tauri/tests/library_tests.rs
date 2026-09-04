fn match_sidecars(target: &std::path::Path, filenames: &[&str]) -> Vec<String> {
    let stem = target.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let target_name = target.file_name().unwrap_or_default().to_string_lossy().to_string();
    let mut matched = Vec::new();

    for fname in filenames {
        if *fname == target_name {
            matched.push(fname.to_string());
            continue;
        }
        let p = std::path::Path::new(fname);
        let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();

        let is_direct_preview = ext == "jpg" && *fname == format!("{stem}.jpg");
        let is_direct_meta = ext == "json" && *fname == format!("{stem}_tracks.json");
        let is_direct_wav = ext == "wav"
            && fname.starts_with(&format!("{stem}_"))
            && !fname.starts_with(&format!("{stem}_edited"));

        if is_direct_preview || is_direct_meta || is_direct_wav {
            matched.push(fname.to_string());
        }
    }
    matched
}

#[test]
fn test_sidecar_filtering_standard_case() {
    let target = std::path::Path::new("C:/Clips/ClipA.mp4");
    let files = vec!["ClipA.mp4", "ClipA.jpg", "ClipA_tracks.json", "ClipA_game.wav"];
    let m = match_sidecars(target, &files);
    assert_eq!(m.len(), 4);
}

#[test]
fn test_sidecar_filtering_protects_edited_mp4() {
    let target = std::path::Path::new("C:/Clips/ClipA.mp4");
    let files = vec!["ClipA.mp4", "ClipA_edited.mp4", "ClipA_edited_2.mp4"];
    let m = match_sidecars(target, &files);
    assert_eq!(m, vec!["ClipA.mp4"]);
}

#[test]
fn test_sidecar_filtering_protects_edited_sidecars() {
    let target = std::path::Path::new("C:/Clips/ClipA.mp4");
    let files = vec![
        "ClipA.mp4",
        "ClipA_edited.jpg",
        "ClipA_edited_tracks.json",
        "ClipA_edited_game.wav",
        "ClipA_edited_mic.wav",
    ];
    let m = match_sidecars(target, &files);
    assert_eq!(m, vec!["ClipA.mp4"]);
}

#[test]
fn test_sidecar_filtering_protects_numbered_prefixes() {
    let target = std::path::Path::new("C:/Clips/Clip1.mp4");
    let files = vec![
        "Clip1.mp4",
        "Clip1.jpg",
        "Clip10.mp4",
        "Clip10.jpg",
        "Clip100.mp4",
        "Clip100_game.wav",
    ];
    let m = match_sidecars(target, &files);
    assert_eq!(m, vec!["Clip1.mp4", "Clip1.jpg"]);
}

#[test]
fn test_sidecar_filtering_with_special_characters() {
    let target = std::path::Path::new("C:/Clips/Game - 2026-08-31 [1080p] #1.mp4");
    let files = vec![
        "Game - 2026-08-31 [1080p] #1.mp4",
        "Game - 2026-08-31 [1080p] #1.jpg",
        "Game - 2026-08-31 [1080p] #1_tracks.json",
        "Game - 2026-08-31 [1080p] #1_game.exe.wav",
        "Game - 2026-08-31 [1080p] #1_edited.mp4",
    ];
    let m = match_sidecars(target, &files);
    assert_eq!(m, vec![
        "Game - 2026-08-31 [1080p] #1.mp4",
        "Game - 2026-08-31 [1080p] #1.jpg",
        "Game - 2026-08-31 [1080p] #1_tracks.json",
        "Game - 2026-08-31 [1080p] #1_game.exe.wav",
    ]);
}

#[test]
fn test_sidecar_filtering_with_dots_in_filename() {
    let target = std::path::Path::new("C:/Clips/Game.v1.0.mp4");
    let files = vec![
        "Game.v1.0.mp4",
        "Game.v1.0.jpg",
        "Game.v1.0_tracks.json",
        "Game.v1.0_game.wav",
        "Game.v1.0_edited.mp4",
    ];
    let m = match_sidecars(target, &files);
    assert_eq!(m, vec![
        "Game.v1.0.mp4",
        "Game.v1.0.jpg",
        "Game.v1.0_tracks.json",
        "Game.v1.0_game.wav",
    ]);
}

#[test]
fn test_sidecar_filtering_multiple_wav_tracks() {
    let target = std::path::Path::new("C:/Clips/ClipA.mp4");
    let files = vec![
        "ClipA.mp4",
        "ClipA.jpg",
        "ClipA_tracks.json",
        "ClipA_System.wav",
        "ClipA_Microphone.wav",
        "ClipA_Discord.exe.wav",
        "ClipA_Spotify.exe.wav",
        "ClipA_Valorant.exe.wav",
    ];
    let m = match_sidecars(target, &files);
    assert_eq!(m.len(), 8);
}

#[test]
fn test_sidecar_filtering_unrelated_files_ignored() {
    let target = std::path::Path::new("C:/Clips/ClipA.mp4");
    let files = vec![
        "ClipA.mp4",
        "ClipB.mp4",
        "ClipB.jpg",
        "random_document.txt",
        "thumbnail.png",
    ];
    let m = match_sidecars(target, &files);
    assert_eq!(m, vec!["ClipA.mp4"]);
}

// ──────────────────────────────────────────────────────────────────────────────
// Advanced Sidecar & Versioning Matching Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_sidecar_filtering_date_formatted_filenames() {
    let target = std::path::Path::new("C:/Clips/Clip_2026-08-31_14-30-00.mp4");
    let files = vec![
        "Clip_2026-08-31_14-30-00.mp4",
        "Clip_2026-08-31_14-30-00.jpg",
        "Clip_2026-08-31_14-30-00_tracks.json",
        "Clip_2026-08-31_14-30-00_Discord.exe.wav",
        "Clip_2026-08-31_14-30-00_edited.mp4", // MUST NOT MATCH
        "Clip_2026-08-31_14-30-00_edited.jpg", // MUST NOT MATCH
    ];
    let m = match_sidecars(target, &files);
    assert_eq!(m.len(), 4);
    assert!(!m.contains(&"Clip_2026-08-31_14-30-00_edited.mp4".to_string()));
    assert!(!m.contains(&"Clip_2026-08-31_14-30-00_edited.jpg".to_string()));
}

#[test]
fn test_sidecar_filtering_protects_multi_version_edits() {
    let target = std::path::Path::new("C:/Clips/Game.mp4");
    let files = vec![
        "Game.mp4",
        "Game.jpg",
        "Game_edited.mp4",
        "Game_edited_2.mp4",
        "Game_edited_3.mp4",
        "Game_edited_2.jpg",
        "Game_edited_2_tracks.json",
        "Game_tracks.json",
    ];
    let m = match_sidecars(target, &files);
    assert_eq!(m.len(), 3);
    assert_eq!(m, vec!["Game.mp4", "Game.jpg", "Game_tracks.json"]);
}

#[test]
fn test_sidecar_filtering_executable_names_with_dots_and_spaces() {
    let target = std::path::Path::new("C:/Clips/Session.mp4");
    let files = vec![
        "Session.mp4",
        "Session_League of Legends.exe.wav",
        "Session_Game.v1.0.4.Beta.exe.wav",
        "Session_UnrealEditor-Win64-Shipping.exe.wav",
    ];
    let m = match_sidecars(target, &files);
    assert_eq!(m.len(), 4);
}

// ──────────────────────────────────────────────────────────────────────────────
// Metadata JSON Serde & Forward Compatibility Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_metadata_json_deserialization_with_spike_markers() {
    let json = r#"{
        "video_file": "ClipA.mp4",
        "duration_secs": 45.5,
        "tracks": [
            { "name": "System", "filename": "ClipA_System.wav", "sample_rate": 48000, "channels": 2 },
            { "name": "Microphone", "filename": "ClipA_Microphone.wav", "sample_rate": 48000, "channels": 1 }
        ],
        "spike_markers": [12.4, 25.8, 38.2]
    }"#;
    let val: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(val["spike_markers"].as_array().unwrap().len(), 3);
    assert_eq!(val["tracks"].as_array().unwrap().len(), 2);
}

#[test]
fn test_metadata_json_legacy_without_spike_markers() {
    let json = r#"{
        "video_file": "LegacyClip.mp4",
        "duration_secs": 30.0,
        "tracks": []
    }"#;
    let val: serde_json::Value = serde_json::from_str(json).unwrap();
    assert!(val.get("spike_markers").is_none());
}

