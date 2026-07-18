//! Optional ONNX embedding engine. Everything degrades to None: no
//! `semantic` feature, no downloaded model, or a failed call all mean
//! "no embeddings" — callers fall back to FTS-only behavior.

#[cfg(feature = "semantic")]
mod imp {
    use agentflare_store::embedding_pipeline::EmbeddingEngine;
    use std::sync::OnceLock;

    fn engine() -> Option<&'static EmbeddingEngine> {
        static ENGINE: OnceLock<Option<EmbeddingEngine>> = OnceLock::new();
        ENGINE
            .get_or_init(|| match EmbeddingEngine::load_default() {
                Ok(e) => Some(e),
                Err(err) => {
                    // One log line per process, not per call.
                    eprintln!("[memory] embedding engine unavailable: {err}");
                    None
                }
            })
            .as_ref()
    }

    pub fn embed_doc(text: &str) -> Option<Vec<f32>> {
        engine()?.embed(text).ok()
    }

    pub fn embed_query(text: &str) -> Option<Vec<f32>> {
        engine()?.embed_query(text).ok()
    }

    pub fn model_name() -> Option<String> {
        Some(engine()?.model_name().to_string())
    }
}

#[cfg(not(feature = "semantic"))]
mod imp {
    pub fn embed_doc(_: &str) -> Option<Vec<f32>> {
        None
    }

    pub fn embed_query(_: &str) -> Option<Vec<f32>> {
        None
    }

    pub fn model_name() -> Option<String> {
        None
    }
}

pub use imp::{embed_doc, embed_query, model_name};
