//! Разрезание длинных аудио-файлов на части ≤ `MAX_UPLOAD_BYTES`.
//! Стратегия: вычисляем `segment_seconds` из bitrate и размера, вызываем `ffmpeg -f segment`.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use super::audio::MAX_UPLOAD_BYTES;

#[derive(Debug)]
pub struct ChunkPlan {
    pub paths: Vec<PathBuf>,
    /// Если всё было нарезано во временный каталог — он будет удалён позже.
    pub temp_dir: Option<PathBuf>,
}

pub fn split_if_needed(audio: &Path) -> Result<ChunkPlan> {
    let size = std::fs::metadata(audio)
        .with_context(|| format!("stat {}", audio.display()))?
        .len();

    if size <= MAX_UPLOAD_BYTES {
        return Ok(ChunkPlan {
            paths: vec![audio.to_path_buf()],
            temp_dir: None,
        });
    }

    let duration = probe_duration_seconds(audio).unwrap_or(0.0);
    // bytes per second; если duration неизвестна — fallback 600 s.
    let bps = if duration > 1.0 {
        size as f64 / duration
    } else {
        size as f64 / 600.0
    };
    let max_seconds_per_chunk = ((MAX_UPLOAD_BYTES as f64) / bps).floor() as u64;
    let segment_seconds = max_seconds_per_chunk.clamp(60, 1500);

    let ext = audio
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("webm")
        .to_string();

    let tmp_dir = create_tmp_dir()?;
    let pattern = tmp_dir.join(format!("part-%03d.{ext}"));

    // Пробуем -c copy; если контейнер не поддерживает сегментирование потока — fallback re-encode в mp3.
    let copy_ok = Command::new("ffmpeg")
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            &audio.to_string_lossy(),
            "-f",
            "segment",
            "-segment_time",
            &segment_seconds.to_string(),
            "-c",
            "copy",
            &pattern.to_string_lossy(),
        ])
        .status()
        .context("spawn ffmpeg segment")?;

    if !copy_ok.success() {
        // fallback: re-encode в mp3
        let fallback_pattern = tmp_dir.join("part-%03d.mp3");
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-i",
                &audio.to_string_lossy(),
                "-vn",
                "-ac",
                "1",
                "-c:a",
                "libmp3lame",
                "-b:a",
                "64k",
                "-f",
                "segment",
                "-segment_time",
                &segment_seconds.to_string(),
                &fallback_pattern.to_string_lossy(),
            ])
            .status()
            .context("spawn ffmpeg segment (mp3 fallback)")?;
        if !status.success() {
            return Err(anyhow!("ffmpeg segment failed even with mp3 fallback"));
        }
    }

    let mut paths = Vec::new();
    for entry in std::fs::read_dir(&tmp_dir)? {
        let p = entry?.path();
        if p.is_file() {
            paths.push(p);
        }
    }
    paths.sort();
    if paths.is_empty() {
        return Err(anyhow!("ffmpeg segment produced no output files"));
    }
    tracing::info!(parts = paths.len(), "audio split into chunks");
    Ok(ChunkPlan {
        paths,
        temp_dir: Some(tmp_dir),
    })
}

fn probe_duration_seconds(audio: &Path) -> Result<f64> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            &audio.to_string_lossy(),
        ])
        .output()
        .context("spawn ffprobe")?;
    if !out.status.success() {
        return Err(anyhow!("ffprobe failed ({})", out.status));
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    s.parse::<f64>().context("parse duration")
}

fn create_tmp_dir() -> Result<PathBuf> {
    let ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("ralume-stt-chunks-{}-{}", std::process::id(), ns));
    std::fs::create_dir_all(&dir).context("create tmp dir")?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_file_is_not_split() {
        let dir = std::env::temp_dir().join(format!("ralume-chunks-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("tiny.webm");
        std::fs::write(&p, b"fake").unwrap();
        let plan = split_if_needed(&p).unwrap();
        assert_eq!(plan.paths.len(), 1);
        assert_eq!(plan.paths[0], p);
        assert!(plan.temp_dir.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
