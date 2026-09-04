use std::collections::HashSet;
use std::path::PathBuf;

// ──────────────────────────────────────────────────────────────────────────────
// Clipboard CF_HDROP & Trash Sidecar Deletion Tests
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_clipboard_dropfiles_wide_path_encoding() {
    let clean_path = "C:\\Users\\Gamer\\Videos\\Clip.mp4";
    let mut wide_path: Vec<u16> = clean_path.encode_utf16().collect();
    wide_path.push(0);
    wide_path.push(0); // Double null-terminated

    // Check last two characters are null terminators
    assert_eq!(wide_path[wide_path.len() - 1], 0);
    assert_eq!(wide_path[wide_path.len() - 2], 0);
    assert_eq!(wide_path[0], 'C' as u16);
    assert_eq!(wide_path[1], ':' as u16);
}

#[test]
fn test_clipboard_canonical_path_prefix_trimming() {
    let raw = r"\\?\C:\Games\Recording.mp4";
    let cleaned = raw.trim_start_matches(r"\\?\");
    assert_eq!(cleaned, "C:\\Games\\Recording.mp4");

    let normal = "C:\\Games\\Recording.mp4";
    let cleaned_normal = normal.trim_start_matches(r"\\?\");
    assert_eq!(cleaned_normal, "C:\\Games\\Recording.mp4");
}

#[test]
fn test_favorites_json_serialization_and_sorting() {
    let mut favs = HashSet::new();
    favs.insert("C:/Videos/Clip3.mp4".to_string());
    favs.insert("C:/Videos/Clip1.mp4".to_string());
    favs.insert("C:/Videos/Clip2.mp4".to_string());

    let mut v: Vec<String> = favs.iter().cloned().collect();
    v.sort();

    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json, r#"["C:/Videos/Clip1.mp4","C:/Videos/Clip2.mp4","C:/Videos/Clip3.mp4"]"#);

    let decoded: Vec<String> = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.len(), 3);
    assert_eq!(decoded[0], "C:/Videos/Clip1.mp4");
}

#[test]
fn test_favorites_toggle_behavior() {
    let mut favs = HashSet::new();
    let clip = "C:/Videos/Game.mp4".to_string();

    // Toggle 1: Add
    let added1 = if favs.contains(&clip) {
        favs.remove(&clip);
        false
    } else {
        favs.insert(clip.clone());
        true
    };
    assert!(added1);
    assert!(favs.contains(&clip));

    // Toggle 2: Remove
    let added2 = if favs.contains(&clip) {
        favs.remove(&clip);
        false
    } else {
        favs.insert(clip.clone());
        true
    };
    assert!(!added2);
    assert!(!favs.contains(&clip));
}

#[test]
fn test_sidecar_victim_discovery_exact_matching() {
    let stem = "2026-08-31_Game_Clip";
    let candidate_files = vec![
        "2026-08-31_Game_Clip.mp4",              // Main video (handled separately)
        "2026-08-31_Game_Clip.jpg",              // Direct preview -> Victim
        "2026-08-31_Game_Clip_tracks.json",      // Direct metadata -> Victim
        "2026-08-31_Game_Clip_System.wav",       // Direct audio track 1 -> Victim
        "2026-08-31_Game_Clip_Microphone.wav",   // Direct audio track 2 -> Victim
        "2026-08-31_Game_Clip_edited.mp4",       // Edited video -> PROTECTED
        "2026-08-31_Game_Clip_edited_tracks.json", // Edited meta -> PROTECTED
        "2026-08-31_Game_Clip2.jpg",             // Sibling clip preview -> PROTECTED
        "Unrelated.mp4",                         // Unrelated -> PROTECTED
    ];

    let mut victims = Vec::new();
    for fname in candidate_files {
        let p = PathBuf::from(fname);
        let ext = p.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
        if fname == format!("{stem}.mp4") {
            continue;
        }

        let is_direct_preview = ext == "jpg" && fname == format!("{stem}.jpg");
        let is_direct_meta = ext == "json" && fname == format!("{stem}_tracks.json");
        let is_direct_wav = ext == "wav"
            && fname.starts_with(&format!("{stem}_"))
            && !fname.starts_with(&format!("{stem}_edited"));

        if is_direct_preview || is_direct_meta || is_direct_wav {
            victims.push(fname);
        }
    }

    assert_eq!(victims.len(), 4);
    assert!(victims.contains(&"2026-08-31_Game_Clip.jpg"));
    assert!(victims.contains(&"2026-08-31_Game_Clip_tracks.json"));
    assert!(victims.contains(&"2026-08-31_Game_Clip_System.wav"));
    assert!(victims.contains(&"2026-08-31_Game_Clip_Microphone.wav"));
    assert!(!victims.contains(&"2026-08-31_Game_Clip_edited.mp4"));
    assert!(!victims.contains(&"2026-08-31_Game_Clip2.jpg"));
}
