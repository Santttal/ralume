use std::process::Command;

use anyhow::{anyhow, Context, Result};

#[derive(Debug, Clone)]
pub struct AudioDevices {
    pub monitor_source: String,
    pub mic_source: String,
}

pub fn detect_audio_devices() -> Result<AudioDevices> {
    let default_sink = pactl(&["get-default-sink"])?;
    if default_sink.is_empty() {
        return Err(anyhow!("pactl returned empty default-sink"));
    }
    let monitor_source = format!("{default_sink}.monitor");

    let mic_source = pactl(&["get-default-source"])?;
    if mic_source.is_empty() {
        return Err(anyhow!("pactl returned empty default-source"));
    }

    // Проверить, что monitor реально присутствует в списке sources.
    let list = pactl(&["list", "short", "sources"])?;
    if !list.lines().any(|line| line.contains(&monitor_source)) {
        tracing::warn!(
            %monitor_source,
            "monitor source not found in `pactl list short sources`, continuing anyway"
        );
    }

    tracing::info!(%monitor_source, %mic_source, "audio devices detected");
    Ok(AudioDevices {
        monitor_source,
        mic_source,
    })
}

/// Поднимает громкость source-устройства до 100%, если она ниже 90%.
/// Нужно для monitor-источников: PulseAudio иногда запоминает их громкость на уровне 20%,
/// и запись идёт, но очень тихая.
pub fn ensure_source_volume_full(source: &str) -> Result<()> {
    let volumes = pactl(&["get-source-volume", source])?;
    let percent = parse_volume_percent(&volumes).unwrap_or(100);
    if percent < 90 {
        tracing::warn!(
            %source,
            current_percent = percent,
            "monitor source volume is low, bumping to 100%"
        );
        pactl(&["set-source-volume", source, "100%"])?;
    } else {
        tracing::debug!(%source, current_percent = percent, "source volume ok");
    }
    Ok(())
}

fn parse_volume_percent(pactl_out: &str) -> Option<u32> {
    // Пример: "Volume: front-left: 13107 /  20% / -41,94 dB,   front-right: ..."
    pactl_out
        .split('%')
        .next()
        .and_then(|seg| seg.rsplit(|c: char| !c.is_ascii_digit()).find(|s| !s.is_empty()))
        .and_then(|n| n.parse::<u32>().ok())
}

fn pactl(args: &[&str]) -> Result<String> {
    let out = Command::new("pactl")
        .args(args)
        .output()
        .with_context(|| format!("spawn pactl {args:?}"))?;
    if !out.status.success() {
        return Err(anyhow!(
            "pactl {args:?} exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8(out.stdout)
        .context("pactl output is not utf-8")?
        .trim()
        .to_owned())
}
