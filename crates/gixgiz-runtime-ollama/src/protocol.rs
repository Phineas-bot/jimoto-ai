use serde::{Deserialize, Serialize};

use crate::error::OllamaAdapterError;

#[derive(Debug, Deserialize)]
pub(crate) struct VersionResponse {
    pub(crate) version: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TagsResponse {
    pub(crate) models: Vec<TagModel>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TagModel {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) model: String,
    pub(crate) size: u64,
    pub(crate) digest: String,
    #[serde(default)]
    pub(crate) remote_model: String,
    #[serde(default)]
    pub(crate) remote_host: String,
}

#[derive(Serialize)]
struct PullRequest<'a> {
    model: &'a str,
    insecure: bool,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct PullResponse {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    digest: Option<String>,
    #[serde(default)]
    total: Option<u64>,
    #[serde(default)]
    completed: Option<u64>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Serialize)]
struct ReadinessRequest<'a> {
    model: &'a str,
    prompt: &'static str,
    stream: bool,
    think: bool,
    keep_alive: u8,
    options: ReadinessOptions,
}

#[derive(Clone, Copy, Serialize)]
struct ReadinessOptions {
    temperature: u8,
    num_predict: u8,
    num_ctx: u16,
}

#[derive(Debug, Deserialize)]
struct ReadinessResponse {
    #[serde(default)]
    response: Option<String>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PullEvent {
    Preparing,
    LayerProgress(PullLayerProgress),
    Verifying,
    Registering,
    Success,
    Ignored,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PullLayerProgress {
    pub(crate) digest: String,
    pub(crate) completed: u64,
    pub(crate) total: u64,
}

const READINESS_PROMPT: &str = "Reply with exactly READY.";
const MAX_PROVIDER_STATUS_BYTES: usize = 128;
const MAX_PROVIDER_FIELD_BYTES: usize = 512;

pub(crate) fn encode_pull_request(model: &str) -> Result<Vec<u8>, OllamaAdapterError> {
    serde_json::to_vec(&PullRequest {
        model,
        insecure: false,
        stream: true,
    })
    .map_err(|_| OllamaAdapterError::Internal)
}

pub(crate) fn decode_pull_event(line: &[u8]) -> Result<PullEvent, OllamaAdapterError> {
    let response: PullResponse =
        serde_json::from_slice(line).map_err(|_| OllamaAdapterError::InvalidResponse)?;
    if let Some(error) = response.error.as_deref() {
        validate_provider_field(error, MAX_PROVIDER_FIELD_BYTES)?;
        if !error.trim().is_empty() {
            return Err(OllamaAdapterError::ModelAcquisitionFailed);
        }
    }
    let status = response
        .status
        .as_deref()
        .ok_or(OllamaAdapterError::InvalidResponse)?;
    validate_provider_field(status, MAX_PROVIDER_STATUS_BYTES)?;

    let normalized = status.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "success" => Ok(PullEvent::Success),
        "verifying sha256 digest" => Ok(PullEvent::Verifying),
        "writing manifest" | "removing any unused layers" => Ok(PullEvent::Registering),
        "pulling manifest" => Ok(PullEvent::Preparing),
        _ => {
            let Some(layer_prefix) = pull_layer_prefix(&normalized) else {
                return Ok(PullEvent::Ignored);
            };
            let digest = response.digest.ok_or(OllamaAdapterError::InvalidResponse)?;
            validate_sha256_digest(&digest)?;
            if !digest
                .strip_prefix("sha256:")
                .and_then(|hex| hex.get(..layer_prefix.len()))
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(layer_prefix))
            {
                return Err(OllamaAdapterError::InvalidResponse);
            }
            let (completed, total) = response
                .completed
                .zip(response.total)
                .ok_or(OllamaAdapterError::InvalidResponse)?;
            if total == 0 || completed > total || total > i64::MAX as u64 {
                return Err(OllamaAdapterError::InvalidResponse);
            }
            Ok(PullEvent::LayerProgress(PullLayerProgress {
                digest,
                completed,
                total,
            }))
        }
    }
}

fn pull_layer_prefix(status: &str) -> Option<&str> {
    status
        .strip_prefix("pulling ")
        .filter(|prefix| prefix.len() == 12 && prefix.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

pub(crate) fn encode_readiness_request(model: &str) -> Result<Vec<u8>, OllamaAdapterError> {
    serde_json::to_vec(&ReadinessRequest {
        model,
        prompt: READINESS_PROMPT,
        stream: false,
        think: false,
        keep_alive: 0,
        options: ReadinessOptions {
            temperature: 0,
            num_predict: 8,
            num_ctx: 512,
        },
    })
    .map_err(|_| OllamaAdapterError::Internal)
}

pub(crate) fn decode_readiness_response(body: &[u8]) -> Result<(), OllamaAdapterError> {
    let response: ReadinessResponse =
        serde_json::from_slice(body).map_err(|_| OllamaAdapterError::InvalidResponse)?;
    if let Some(error) = response.error.as_deref() {
        validate_provider_field(error, MAX_PROVIDER_FIELD_BYTES)?;
        if !error.trim().is_empty() {
            return Err(OllamaAdapterError::ReadinessFailed);
        }
    }
    let valid_response = response
        .response
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    if !response.done || !valid_response {
        return Err(OllamaAdapterError::ReadinessFailed);
    }
    Ok(())
}

fn validate_provider_field(value: &str, max_bytes: usize) -> Result<(), OllamaAdapterError> {
    if value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(OllamaAdapterError::InvalidResponse);
    }
    Ok(())
}

pub(crate) fn decode_version(body: &[u8]) -> Result<VersionResponse, OllamaAdapterError> {
    let response: VersionResponse =
        serde_json::from_slice(body).map_err(|_| OllamaAdapterError::InvalidResponse)?;
    if response.version.trim().is_empty() || response.version.len() > 64 {
        return Err(OllamaAdapterError::InvalidResponse);
    }
    Ok(response)
}

pub(crate) fn decode_tags(
    body: &[u8],
    max_models: usize,
) -> Result<TagsResponse, OllamaAdapterError> {
    let response: TagsResponse =
        serde_json::from_slice(body).map_err(|_| OllamaAdapterError::InvalidResponse)?;
    if response.models.len() > max_models {
        return Err(OllamaAdapterError::ResponseTooLarge);
    }
    for model in &response.models {
        validate_model(model)?;
    }
    Ok(response)
}

fn validate_model(model: &TagModel) -> Result<(), OllamaAdapterError> {
    const MAX_FIELD_LENGTH: usize = 512;
    let fields = [
        model.name.as_str(),
        model.model.as_str(),
        model.digest.as_str(),
        model.remote_model.as_str(),
        model.remote_host.as_str(),
    ];
    if model.name.trim().is_empty()
        || fields
            .iter()
            .any(|field| field.len() > MAX_FIELD_LENGTH || field.chars().any(char::is_control))
    {
        return Err(OllamaAdapterError::InvalidResponse);
    }
    validate_sha256_digest(&model.digest)?;
    Ok(())
}

fn validate_sha256_digest(value: &str) -> Result<(), OllamaAdapterError> {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return Err(OllamaAdapterError::InvalidResponse);
    };
    if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(OllamaAdapterError::InvalidResponse);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn additive_unknown_fields_are_accepted() {
        let response = decode_tags(
            br#"{"models":[{"name":"qwen2.5:0.5b-instruct","model":"qwen2.5:0.5b-instruct","modified_at":"2026-01-01T00:00:00Z","size":123,"digest":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","future_field":true,"details":{"format":"gguf","future_detail":7}}]}"#,
            10,
        )
        .expect("additive fields are ignored");

        assert_eq!(response.models.len(), 1);
    }

    #[test]
    fn malformed_or_oversized_model_lists_fail_closed() {
        assert_eq!(
            decode_tags(br#"{"models":[{"name":"","size":1,"digest":"abc"}]}"#, 10).unwrap_err(),
            OllamaAdapterError::InvalidResponse
        );
        assert_eq!(
            decode_tags(
                br#"{"models":[{"name":"one","size":1,"digest":"a"},{"name":"two","size":2,"digest":"b"}]}"#,
                1,
            )
            .unwrap_err(),
            OllamaAdapterError::ResponseTooLarge
        );
    }

    #[test]
    fn model_fields_with_control_characters_fail_closed() {
        assert_eq!(
            decode_tags(
                br#"{"models":[{"name":"unsafe\nname","size":1,"digest":"abc"}]}"#,
                10,
            )
            .unwrap_err(),
            OllamaAdapterError::InvalidResponse
        );
    }

    #[test]
    fn pull_layer_progress_keeps_validated_provider_identity_private() {
        let event = decode_pull_event(
            br#"{"status":"pulling 0123456789ab","digest":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","total":400,"completed":100}"#,
        )
        .expect("documented pull event");

        assert_eq!(
            event,
            PullEvent::LayerProgress(PullLayerProgress {
                digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                    .to_owned(),
                completed: 100,
                total: 400,
            })
        );
    }

    #[test]
    fn pull_error_and_invalid_counters_fail_closed_without_exposing_text() {
        assert_eq!(
            decode_pull_event(br#"{"error":"private provider detail"}"#)
                .expect_err("provider error rejected"),
            OllamaAdapterError::ModelAcquisitionFailed
        );
        assert_eq!(
            decode_pull_event(
                br#"{"status":"pulling 0123456789ab","digest":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","total":100,"completed":101}"#,
            )
            .expect_err("invalid counters rejected"),
            OllamaAdapterError::InvalidResponse
        );
    }

    #[test]
    fn additive_unknown_pull_status_is_ignored_with_its_unrecognized_fields() {
        let event = decode_pull_event(
            br#"{"status":"pulling future metadata","digest":"not-a-digest","total":1,"completed":2,"future":{"private":"discarded"}}"#,
        )
        .expect("bounded additive status is ignored");

        assert_eq!(event, PullEvent::Ignored);
    }

    #[test]
    fn unsupported_or_malformed_integrity_evidence_fails_closed() {
        for digest in [
            "abc",
            "sha256:short",
            "sha512:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdeg",
        ] {
            let line = format!(
                r#"{{"status":"pulling 0123456789ab","digest":"{digest}","total":100,"completed":50}}"#
            );
            assert_eq!(
                decode_pull_event(line.as_bytes()).err(),
                Some(OllamaAdapterError::InvalidResponse)
            );
        }
        assert_eq!(
            decode_pull_event(
                br#"{"status":"pulling 0123456789ab","digest":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","total":100}"#,
            ),
            Err(OllamaAdapterError::InvalidResponse)
        );
        assert_eq!(
            decode_pull_event(
                br#"{"status":"pulling fedcba987654","digest":"sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef","total":100,"completed":50}"#,
            ),
            Err(OllamaAdapterError::InvalidResponse)
        );
    }

    #[test]
    fn readiness_request_is_fixed_bounded_and_response_content_is_discarded() {
        let encoded =
            encode_readiness_request("qwen2.5:0.5b-instruct").expect("fixed request serializes");
        let value: serde_json::Value =
            serde_json::from_slice(&encoded).expect("fixed request parses");

        assert_eq!(value["stream"], false);
        assert_eq!(value["think"], false);
        assert_eq!(value["keep_alive"], 0);
        assert_eq!(value["options"]["num_predict"], 8);
        decode_readiness_response(br#"{"response":"READY","done":true}"#)
            .expect("valid non-empty response");
        assert_eq!(
            decode_readiness_response(br#"{"response":"","done":true}"#),
            Err(OllamaAdapterError::ReadinessFailed)
        );
    }

    #[test]
    fn pull_request_disables_insecure_registry_access() {
        let encoded =
            encode_pull_request("qwen2.5:0.5b-instruct").expect("fixed pull request serializes");
        let value: serde_json::Value =
            serde_json::from_slice(&encoded).expect("pull request parses");

        assert_eq!(value["insecure"], false);
        assert_eq!(value["stream"], true);
    }
}
