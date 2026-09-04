fn main() {
    tauri_build::build();

    // Copy sidecars to target directory so running standalone target/release/clip-tool.exe always finds ffmpeg
    if let Ok(out_dir) = std::env::var("OUT_DIR") {
        let out_path = std::path::PathBuf::from(out_dir);
        if let Some(target_dir) = out_path.ancestors().nth(3) {
            let bin_dir = std::path::Path::new("bin");
            if bin_dir.exists() {
                let _ = std::fs::copy(
                    bin_dir.join("ffmpeg-x86_64-pc-windows-msvc.exe"),
                    target_dir.join("ffmpeg.exe"),
                );
                let _ = std::fs::copy(
                    bin_dir.join("ffmpeg-x86_64-pc-windows-msvc.exe"),
                    target_dir.join("ffmpeg-x86_64-pc-windows-msvc.exe"),
                );
                let _ = std::fs::copy(
                    bin_dir.join("ffprobe-x86_64-pc-windows-msvc.exe"),
                    target_dir.join("ffprobe.exe"),
                );
                let _ = std::fs::copy(
                    bin_dir.join("ffprobe-x86_64-pc-windows-msvc.exe"),
                    target_dir.join("ffprobe-x86_64-pc-windows-msvc.exe"),
                );
            }
        }
    }
}
