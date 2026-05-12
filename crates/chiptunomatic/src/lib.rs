//! Procedural chiptune music from arbitrary bytes — same input yields the same audio.
//! Core synthesis has no mandatory file or audio-device I/O so it builds for `wasm32-unknown-unknown`
//! as well as native targets; [`SongMetadata::from_path`] is omitted on **`wasm32-unknown-unknown`**
//! (browser) where `std::fs` metadata is unavailable—use [`SongMetadata::from_string`] there.
//!
//! ## WebAssembly
//!
//! Use the **`chiptunomatic-wasm`** crate for wasm-bindgen bindings (incremental synth + metadata for JS). It
//! depends on this library and is built with:
//!
//! ```text
//! rustup target add wasm32-unknown-unknown
//! cargo build -p chiptunomatic-wasm --target wasm32-unknown-unknown
//! wasm-bindgen target/wasm32-unknown-unknown/release/chiptunomatic_wasm.wasm --out-dir pkg --target web
//! ```
//!
//! On `wasm32`, enable this crate's **`js`** feature so `getrandom` is built with its **`js`**
//! backend (the **`chiptunomatic-wasm`** crate does that). Without **`js`**, a wasm32 build
//! must supply another `getrandom` backend. Use a JavaScript-capable host when **`js`** is on.

#![no_std]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod constants;
mod drum;
mod metadata;
mod mix;
mod note;
mod song;
pub mod synth;
#[cfg(feature = "wav")]
mod wav;

pub use drum::*;
pub use metadata::*;
pub use mix::*;
pub use note::*;
pub use song::*;
#[cfg(feature = "wav")]
pub use wav::*;
