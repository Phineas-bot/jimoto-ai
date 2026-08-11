use serde::Deserialize;

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
        || model.digest.trim().is_empty()
        || fields
            .iter()
            .any(|field| field.len() > MAX_FIELD_LENGTH || field.chars().any(char::is_control))
    {
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
            br#"{"models":[{"name":"qwen2.5:0.5b-instruct","model":"qwen2.5:0.5b-instruct","modified_at":"2026-01-01T00:00:00Z","size":123,"digest":"abc","future_field":true,"details":{"format":"gguf","future_detail":7}}]}"#,
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
}
