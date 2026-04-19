//! reqwest-клиент к `POST /v1/audio/transcriptions`.

use std::path::Path;
use std::time::Duration;

use reqwest::multipart;

use super::TranscriptionError;
use crate::config::TranscriptionModel;

pub async fn upload_with_retry(
    client: &reqwest::Client,
    file: &Path,
    api_key: &str,
    model: TranscriptionModel,
    language: &str,
    attempts: u32,
) -> Result<String, TranscriptionError> {
    let mut last_err = TranscriptionError::Http("no attempts".into());
    for attempt in 0..attempts.max(1) {
        match upload_single(client, file, api_key, model, language).await {
            Ok(text) => return Ok(text),
            Err(e) => {
                if !is_retryable(&e) || attempt + 1 == attempts {
                    return Err(e);
                }
                let delay = backoff_secs(attempt);
                tracing::warn!(
                    attempt = attempt + 1,
                    attempts,
                    delay,
                    err = %e,
                    "transcription attempt failed, retrying"
                );
                tokio::time::sleep(Duration::from_secs(delay)).await;
                last_err = e;
            }
        }
    }
    Err(last_err)
}

fn is_retryable(err: &TranscriptionError) -> bool {
    match err {
        TranscriptionError::Api { status, .. } => {
            matches!(*status, 429 | 500 | 502 | 503 | 504)
        }
        TranscriptionError::Http(_) => true, // сеть — обычно временная
        _ => false,
    }
}

fn backoff_secs(attempt: u32) -> u64 {
    // 1s, 2s, 4s ...
    1u64 << attempt
}

async fn upload_single(
    client: &reqwest::Client,
    file_path: &Path,
    api_key: &str,
    model: TranscriptionModel,
    language: &str,
) -> Result<String, TranscriptionError> {
    let bytes = tokio::fs::read(file_path).await?;
    let file_name = file_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".into());

    let part = multipart::Part::bytes(bytes)
        .file_name(file_name)
        .mime_str(mime_for(file_path))
        .map_err(|e| TranscriptionError::Http(e.to_string()))?;

    let response_format = if matches!(model, TranscriptionModel::Gpt4oTranscribeDiarize) {
        "diarized_json"
    } else if model.supports_text_response() {
        "text"
    } else {
        "json"
    };

    let mut form = multipart::Form::new()
        .text("model", model.api_id())
        .text("response_format", response_format);
    if !language.is_empty() {
        form = form.text("language", language.to_owned());
    }
    // diarize-модели требуют явный chunking_strategy (invalid_request без него).
    if matches!(model, TranscriptionModel::Gpt4oTranscribeDiarize) {
        form = form.text("chunking_strategy", "auto");
    }
    form = form.part("file", part);

    let resp = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| TranscriptionError::Http(e.to_string()))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| TranscriptionError::Http(e.to_string()))?;

    if !status.is_success() {
        return Err(TranscriptionError::Api {
            status: status.as_u16(),
            body,
        });
    }

    if model.supports_text_response() {
        return Ok(body);
    }

    let v: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| TranscriptionError::Http(format!("bad json: {e}")))?;

    if matches!(model, TranscriptionModel::Gpt4oTranscribeDiarize) {
        if let Some(dialogue) = format_diarized(&v) {
            return Ok(dialogue);
        }
        tracing::warn!(body = %body.chars().take(300).collect::<String>(), "diarized_json parse fallback");
    }

    if let Some(t) = v.get("text").and_then(|x| x.as_str()) {
        if !t.is_empty() {
            return Ok(t.to_owned());
        }
    }
    if let Some(segments) = v.get("segments").and_then(|x| x.as_array()) {
        let joined: Vec<String> = segments
            .iter()
            .filter_map(|s| s.get("text").and_then(|x| x.as_str()).map(|s| s.to_owned()))
            .collect();
        if !joined.is_empty() {
            return Ok(joined.join("\n"));
        }
    }
    Ok(String::new())
}

/// Форматирует ответ diarized_json как диалог `Speaker X: …`.
/// Поддерживает обычные поля `segments[]` с `speaker`+`text`; соседние
/// реплики одного спикера склеиваются в один абзац.
fn format_diarized(v: &serde_json::Value) -> Option<String> {
    let segments = v
        .get("segments")
        .and_then(|x| x.as_array())
        .or_else(|| v.get("results").and_then(|x| x.as_array()))?;
    if segments.is_empty() {
        return None;
    }

    let mut out: Vec<String> = Vec::new();
    let mut cur_speaker: Option<String> = None;
    let mut cur_text = String::new();

    for seg in segments {
        let speaker = seg
            .get("speaker")
            .and_then(|s| s.as_str())
            .map(|s| s.to_owned())
            .or_else(|| {
                seg.get("speaker_id")
                    .and_then(|s| s.as_i64())
                    .map(|i| format!("speaker_{i}"))
            })
            .unwrap_or_else(|| "speaker".to_owned());
        let text = seg
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .trim()
            .to_owned();
        if text.is_empty() {
            continue;
        }

        if cur_speaker.as_ref() == Some(&speaker) {
            cur_text.push(' ');
            cur_text.push_str(&text);
        } else {
            if let Some(s) = cur_speaker.take() {
                out.push(format!("{}: {}", pretty_speaker(&s), cur_text.trim()));
            }
            cur_speaker = Some(speaker);
            cur_text = text;
        }
    }
    if let Some(s) = cur_speaker {
        out.push(format!("{}: {}", pretty_speaker(&s), cur_text.trim()));
    }

    if out.is_empty() {
        None
    } else {
        Some(out.join("\n\n"))
    }
}

/// `speaker_0` → `Speaker 1`, `speaker_1` → `Speaker 2` и т.д. Чтобы нумерация
/// в `.txt` была человеко-удобной.
fn pretty_speaker(raw: &str) -> String {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    match digits.parse::<u32>() {
        Ok(n) => format!("Speaker {}", n + 1),
        Err(_) => raw.to_owned(),
    }
}

fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "mp3" => "audio/mpeg",
        "m4a" | "mp4" => "audio/mp4",
        "webm" => "audio/webm",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "ogg" => "audio/ogg",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles() {
        assert_eq!(backoff_secs(0), 1);
        assert_eq!(backoff_secs(1), 2);
        assert_eq!(backoff_secs(2), 4);
    }

    #[test]
    fn diarized_parses_as_dialogue() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{"segments":[
                {"speaker":"speaker_0","text":"Привет."},
                {"speaker":"speaker_0","text":"Как дела?"},
                {"speaker":"speaker_1","text":"Нормально."},
                {"speaker":"speaker_0","text":"Хорошо."}
            ]}"#,
        )
        .unwrap();
        let out = format_diarized(&v).unwrap();
        assert_eq!(
            out,
            "Speaker 1: Привет. Как дела?\n\nSpeaker 2: Нормально.\n\nSpeaker 1: Хорошо."
        );
    }

    #[test]
    fn retryable_on_429_and_5xx() {
        let err = TranscriptionError::Api {
            status: 429,
            body: "".into(),
        };
        assert!(is_retryable(&err));
        let err = TranscriptionError::Api {
            status: 503,
            body: "".into(),
        };
        assert!(is_retryable(&err));
        let err = TranscriptionError::Api {
            status: 401,
            body: "".into(),
        };
        assert!(!is_retryable(&err));
    }
}
