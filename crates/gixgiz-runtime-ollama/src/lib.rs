#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Windows-first Ollama runtime adapter with direct loopback transport and bounded operations.

mod adapter;
mod chat_http;
mod discovery;
mod endpoint;
mod error;
mod http;
mod installer;
mod model_http;
mod models;
mod process;
mod protocol;
mod storage;
mod transfer;
mod version;

pub use adapter::{OLLAMA_PROVIDER_ID, OllamaAdapter};
pub use installer::OllamaInstaller;
/// Host-facing name for the concrete v0.1 runtime provider.
pub type OllamaRuntimeProvider = OllamaAdapter;
