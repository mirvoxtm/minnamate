pub mod arrange;
pub mod display;
pub mod doc;
pub mod draw;
pub mod edit;
pub mod erase;
pub mod fonts;
pub mod hit;
pub mod paint;
pub mod pen;
pub mod render;
pub mod swf_export;
pub mod symbols;
pub mod text;
pub mod transform;
pub mod xfl;
pub mod xfl_write;

pub use tiny_skia;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
