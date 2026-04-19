//! OpenAI speech-to-text (Фаза 12).
//!
//! `transcribe_file(video, &Settings)` делает три вещи:
//! 1. Готовит аудио-файл под API-лимит (remux/re-encode через ffmpeg).
//! 2. Делит на части ≤ 24 МБ, если надо.
//! 3. Грузит каждую часть на `POST /v1/audio/transcriptions` и склеивает текст.

pub mod audio;
pub mod chunks;
pub mod client;

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::{Settings, TranscriptionModel};

#[derive(Debug, thiserror::Error)]
pub enum TranscriptionError {
    #[error("OpenAI API key is empty")]
    NoApiKey,
    #[error("audio preparation failed: {0}")]
    AudioPrep(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("api error {status}: {body}")]
    Api { status: u16, body: String },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub struct TranscriptionResult {
    pub text: String,
    pub model: TranscriptionModel,
    pub chunks: u32,
}

pub async fn transcribe_file(
    video_path: &Path,
    settings: &Settings,
    progress: Option<&async_channel::Sender<(u32, u32)>>,
) -> Result<TranscriptionResult, TranscriptionError> {
    if settings.openai_api_key.trim().is_empty() {
        return Err(TranscriptionError::NoApiKey);
    }

    let prepared = audio::prepare_audio_for_upload(video_path)
        .map_err(|e| TranscriptionError::AudioPrep(e.to_string()))?;

    let parts = chunks::split_if_needed(&prepared.path)
        .map_err(|e| TranscriptionError::AudioPrep(e.to_string()))?;

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| TranscriptionError::Http(e.to_string()))?;

    let key = settings.openai_api_key.trim();
    let lang = settings.transcription_language.trim();
    let model = settings.transcription_model;
    let total = parts.paths.len() as u32;

    let mut collected = Vec::with_capacity(parts.paths.len());
    for (i, part) in parts.paths.iter().enumerate() {
        let part_no = i as u32 + 1;
        tracing::info!(
            part = part_no,
            total,
            path = %part.display(),
            "uploading part"
        );
        if let Some(tx) = progress {
            let _ = tx.send((part_no, total)).await;
        }
        let text = client::upload_with_retry(&http, part, key, model, lang, 3).await?;
        collected.push(text);
    }

    cleanup_tempfiles(&prepared, &parts);

    Ok(TranscriptionResult {
        text: collected.join("\n\n"),
        model,
        chunks: total,
    })
}

fn cleanup_tempfiles(prepared: &audio::PreparedAudio, parts: &chunks::ChunkPlan) {
    if let Some(dir) = &parts.temp_dir {
        if let Err(e) = std::fs::remove_dir_all(dir) {
            tracing::warn!(%e, path = %dir.display(), "failed to remove chunks tmp dir");
        }
    }
    if prepared.is_temporary {
        if let Err(e) = std::fs::remove_file(&prepared.path) {
            tracing::warn!(%e, path = %prepared.path.display(), "failed to remove prepared audio");
        }
    }
}

/// UI-friendly расшифровка ошибки — чтобы UI не знал про варианты enum.
pub fn friendly_message(err: &TranscriptionError) -> String {
    match err {
        TranscriptionError::NoApiKey => "Укажите API-ключ OpenAI в Настройках".into(),
        TranscriptionError::AudioPrep(e) => format!("Не удалось подготовить аудио: {e}"),
        TranscriptionError::Http(e) => {
            if e.to_lowercase().contains("dns") || e.contains("failed to lookup") {
                "Сеть недоступна — проверьте интернет-соединение".into()
            } else {
                format!("Сеть: {e}")
            }
        }
        TranscriptionError::Api { status: 401, .. } => "Неверный API-ключ OpenAI".into(),
        TranscriptionError::Api { status: 403, .. } => {
            "Доступ запрещён — проверьте права ключа OpenAI".into()
        }
        TranscriptionError::Api { status: 413, .. } => {
            "Запись слишком большая для загрузки даже после чанкинга".into()
        }
        TranscriptionError::Api { status: 429, .. } => {
            "Превышен лимит запросов OpenAI — попробуйте позже".into()
        }
        TranscriptionError::Api { status, .. } if *status >= 500 => {
            "Сервер OpenAI временно недоступен".into()
        }
        TranscriptionError::Api { status, body } => {
            let tail = body.chars().take(120).collect::<String>();
            format!("Ошибка API {status}: {tail}")
        }
        TranscriptionError::Io(e) => format!("Ошибка записи файла: {e}"),
    }
}

// Доступ для UI-обёрток, которые строят путь к .txt.
pub fn text_output_path(video_path: &Path) -> PathBuf {
    video_path.with_extension("txt")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friendly_messages_cover_common_cases() {
        assert!(friendly_message(&TranscriptionError::NoApiKey).contains("API-ключ"));
        assert!(friendly_message(&TranscriptionError::Api {
            status: 401,
            body: "bad".into()
        })
        .contains("Неверный"));
        assert!(friendly_message(&TranscriptionError::Api {
            status: 503,
            body: "".into()
        })
        .contains("недоступен"));
    }

    #[test]
    fn text_output_path_swaps_extension() {
        let p = PathBuf::from("/tmp/test.mp4");
        assert_eq!(text_output_path(&p), PathBuf::from("/tmp/test.txt"));
    }
}
