//! Локальная «библиотека» записей — файловый сканер + кеш thumbnail'ов.
//! Phase 19.b: без SQLite, источник правды — файлы в `settings.output_dir`.

#![allow(dead_code, unused_imports)]

pub mod scanner;
pub mod thumbs;

pub use scanner::{enrich, scan, Recording};
pub use thumbs::{ensure_thumb, thumb_path};
