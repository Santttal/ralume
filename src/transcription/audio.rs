//! Подготовка аудио-файла для OpenAI STT.
//!
//! Стратегия: всегда перекодируем в `webm/Opus mono 16 kHz VBR @ 24 kb/s
//! application=voip` — это ~133 минуты в 24 МБ при near-baseline качестве
//! распознавания. Whisper внутри всё равно делает downmix→mono и
//! resample→16 kHz, поэтому слать больше бессмысленно.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};

pub const MAX_UPLOAD_BYTES: u64 = 24 * 1024 * 1024;

/// Целевой битрейт в kbps. Opus VBR voip — оптимум для speech.
const TARGET_KBPS: &str = "24";

#[derive(Debug)]
pub struct PreparedAudio {
    pub path: PathBuf,
    pub is_temporary: bool,
}

pub fn prepare_audio_for_upload(video: &Path) -> Result<PreparedAudio> {
    let out = tempfile_with_ext("webm");
    run_ffmpeg(&[
        "-y", "-hide_banner", "-loglevel", "error",
        "-i", &video.to_string_lossy(),
        "-vn",
        "-c:a", "libopus",
        "-ac", "1",
        "-ar", "16000",
        "-b:a", &format!("{TARGET_KBPS}k"),
        "-vbr", "on",
        "-application", "voip",
        "-frame_duration", "20",
        &out.to_string_lossy(),
    ])
    .context("ffmpeg encode to opus 24k mono 16k")?;

    let size = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(u64::MAX);
    tracing::info!(path = %out.display(), size, "audio encoded to opus mono 16k 24kbps");
    Ok(PreparedAudio { path: out, is_temporary: true })
}

fn run_ffmpeg(args: &[&str]) -> Result<()> {
    let out = Command::new("ffmpeg")
        .args(args)
        .output()
        .context("spawn ffmpeg (not installed?)")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!(
            "ffmpeg failed ({}): {}",
            out.status,
            stderr.lines().last().unwrap_or("").trim()
        ));
    }
    Ok(())
}

fn tempfile_with_ext(ext: &str) -> PathBuf {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "ralume-stt-{}-{}.{}",
        std::process::id(),
        ns,
        ext
    ))
}
