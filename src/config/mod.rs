//! Config-модуль: пользовательские настройки (serde + toml).

pub mod settings;

pub use settings::{
    load, save, shared, AudioMode, Container, CursorMode, EncoderHint, RegionMode, Settings,
    SharedSettings, VideoCodec,
};
