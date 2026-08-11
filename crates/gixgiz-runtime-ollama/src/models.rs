use crate::protocol::TagModel;

const KNOWN_MODEL_TAGS: [(&str, &str); 6] = [
    ("qwen2.5:0.5b-instruct", "qwen2.5.0.5b-instruct"),
    ("qwen2.5:1.5b-instruct", "qwen2.5.1.5b-instruct"),
    ("qwen2.5:7b-instruct", "qwen2.5.7b-instruct"),
    ("qwen2.5-coder:0.5b-instruct", "qwen2.5-coder.0.5b-instruct"),
    ("qwen2.5-coder:1.5b-instruct", "qwen2.5-coder.1.5b-instruct"),
    ("qwen2.5-coder:7b-instruct", "qwen2.5-coder.7b-instruct"),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MappedModel {
    pub(crate) provider_id: String,
    pub(crate) canonical_id: Option<String>,
    pub(crate) size_bytes: u64,
    pub(crate) is_remote: bool,
}

pub(crate) fn map_model(model: TagModel) -> MappedModel {
    let provider_id = if model.model.trim().is_empty() {
        model.name
    } else {
        model.model
    };
    let canonical_id = KNOWN_MODEL_TAGS
        .iter()
        .find(|(tag, _)| provider_id.eq_ignore_ascii_case(tag))
        .map(|(_, id)| (*id).to_owned());

    MappedModel {
        provider_id,
        canonical_id,
        size_bytes: model.size,
        is_remote: !model.remote_host.trim().is_empty() || !model.remote_model.trim().is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag_model(provider_id: &str) -> TagModel {
        TagModel {
            name: provider_id.to_owned(),
            model: provider_id.to_owned(),
            size: 123,
            digest: "digest".to_owned(),
            remote_model: String::new(),
            remote_host: String::new(),
        }
    }

    #[test]
    fn every_approved_tag_maps_to_its_catalogue_identity() {
        for (provider_id, catalogue_id) in KNOWN_MODEL_TAGS {
            let mapped = map_model(tag_model(provider_id));

            assert_eq!(mapped.provider_id, provider_id);
            assert_eq!(mapped.canonical_id.as_deref(), Some(catalogue_id));
        }
    }

    #[test]
    fn unknown_tag_remains_explicitly_unknown() {
        let mapped = map_model(tag_model("custom/private:latest"));

        assert_eq!(mapped.canonical_id, None);
        assert_eq!(mapped.provider_id, "custom/private:latest");
    }

    #[test]
    fn remote_model_is_marked_without_following_its_host() {
        let mut model = tag_model("custom:cloud");
        model.remote_host = "https://example.invalid".to_owned();

        assert!(map_model(model).is_remote);
    }
}
