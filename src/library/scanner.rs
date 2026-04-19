//! Сканер `settings.output_dir` — находит видео-файлы и собирает метаданные
//! для отображения в Library-экране (phase 19.b.2).
//!
//! Ffprobe — опциональный: если не установлен, длительность и разрешение
//! пропускаются, карточка показывает «—».

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

use chrono::{DateTime, Local};

/// Расширения, которые мы считаем записями Ralume.
const VIDEO_EXTS: &[&str] = &["mp4", "mkv", "webm"];

#[derive(Debug, Clone)]
pub struct Recording {
    pub path: PathBuf,
    pub title: String,
    pub created: DateTime<Local>,
    pub size_bytes: u64,
    /// Продолжительность в секундах (из ffprobe). `None` если ffprobe недоступен.
    pub duration_seconds: Option<f64>,
    /// Разрешение (width, height). `None` если ffprobe недоступен.
    pub resolution: Option<(u32, u32)>,
    /// Соседний файл `<stem>.txt` существует.
    pub has_transcript: bool,
}

impl Recording {
    pub fn duration_display(&self) -> String {
        match self.duration_seconds {
            Some(secs) => format_duration(secs as u64),
            None => "—".to_owned(),
        }
    }

    pub fn resolution_display(&self) -> String {
        match self.resolution {
            Some((w, h)) => format!("{w}×{h}"),
            None => "—".to_owned(),
        }
    }

    pub fn size_display(&self) -> String {
        format_size(self.size_bytes)
    }

    pub fn date_display(&self) -> String {
        self.created.format("%d.%m.%Y").to_string()
    }
}

/// Сканирует директорию. Возвращает записи, отсортированные от новых к старым.
/// Не рекурсивно.
pub fn scan(dir: &Path) -> Vec<Recording> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<Recording> = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        if !is_video(&path) {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let created: DateTime<Local> = meta
            .modified()
            .or_else(|_| meta.created())
            .unwrap_or(SystemTime::UNIX_EPOCH)
            .into();
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Без названия".to_owned());
        let has_transcript =
            path.with_extension("txt").is_file() || path.with_extension("json").is_file();

        let (duration_seconds, resolution) = ffprobe_meta(&path).unwrap_or((None, None));

        out.push(Recording {
            path,
            title,
            created,
            size_bytes: meta.len(),
            duration_seconds,
            resolution,
            has_transcript,
        });
    }
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

fn is_video(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    VIDEO_EXTS.contains(&ext.to_ascii_lowercase().as_str())
}

/// Читает через `ffprobe` длительность и разрешение. Любая ошибка → `None`.
fn ffprobe_meta(path: &Path) -> Option<(Option<f64>, Option<(u32, u32)>)> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "format=duration:stream=width,height",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut duration: Option<f64> = None;
    let mut width: Option<u32> = None;
    let mut height: Option<u32> = None;
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "duration" => duration = v.trim().parse().ok(),
            "width" => width = v.trim().parse().ok(),
            "height" => height = v.trim().parse().ok(),
            _ => {}
        }
    }
    let res = match (width, height) {
        (Some(w), Some(h)) => Some((w, h)),
        _ => None,
    };
    Some((duration, res))
}

fn format_duration(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{} KB", bytes / KB)
    } else {
        format!("{bytes} B")
    }
}
