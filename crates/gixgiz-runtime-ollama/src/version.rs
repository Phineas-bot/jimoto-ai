use semver::Version;

use crate::error::OllamaAdapterError;

/// Evidence-backed support assessment. Versions outside recorded evidence are not rejected
/// automatically, but they cannot be reported as compatible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VersionSupport {
    Compatible,
    Untested,
    Incompatible,
}

/// GixGiz support policy, independent from Ollama's release cadence.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct VersionPolicy {
    minimum_supported: Option<Version>,
    tested_through: Option<Version>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InvalidVersionRange;

impl VersionPolicy {
    #[cfg(test)]
    #[must_use]
    pub(crate) fn capability_first() -> Self {
        Self::default()
    }

    #[must_use]
    pub(crate) fn v0_1() -> Self {
        Self::with_tested_range(Version::new(0, 12, 6), Version::new(0, 32, 5))
            .expect("hard-coded v0.1 supported version range must be ordered")
    }

    pub(crate) fn with_tested_range(
        minimum_supported: Version,
        tested_through: Version,
    ) -> Result<Self, InvalidVersionRange> {
        if minimum_supported > tested_through {
            return Err(InvalidVersionRange);
        }
        Ok(Self {
            minimum_supported: Some(minimum_supported),
            tested_through: Some(tested_through),
        })
    }

    pub(crate) fn assess(
        &self,
        raw: &str,
    ) -> Result<(Version, VersionSupport), OllamaAdapterError> {
        let version = parse_version(raw)?;
        let support = match (&self.minimum_supported, &self.tested_through) {
            _ if !version.pre.is_empty() => VersionSupport::Untested,
            (Some(minimum), Some(_)) if &version < minimum => VersionSupport::Incompatible,
            (Some(_), Some(tested_through)) if &version <= tested_through => {
                VersionSupport::Compatible
            }
            _ => VersionSupport::Untested,
        };
        Ok((version, support))
    }
}

pub(crate) fn parse_version(raw: &str) -> Result<Version, OllamaAdapterError> {
    let normalized = raw.trim().strip_prefix('v').unwrap_or(raw.trim());
    Version::parse(normalized).map_err(|_| OllamaAdapterError::InvalidVersion)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_optional_v_prefix_and_preserves_prerelease_uncertainty() {
        assert_eq!(parse_version(" v0.12.6 ").unwrap(), Version::new(0, 12, 6));
        let policy =
            VersionPolicy::with_tested_range(Version::new(0, 10, 0), Version::new(0, 20, 0))
                .unwrap();

        assert_eq!(
            policy.assess("0.12.6-rc.1").unwrap().1,
            VersionSupport::Untested
        );
    }

    #[test]
    fn tested_range_distinguishes_old_and_future_versions() {
        let policy =
            VersionPolicy::with_tested_range(Version::new(0, 10, 0), Version::new(0, 20, 0))
                .unwrap();

        assert_eq!(
            policy.assess("0.9.9").unwrap().1,
            VersionSupport::Incompatible
        );
        assert_eq!(
            policy.assess("0.12.6").unwrap().1,
            VersionSupport::Compatible
        );
        assert_eq!(policy.assess("1.0.0").unwrap().1, VersionSupport::Untested);
    }

    #[test]
    fn capability_first_policy_does_not_invent_release_bounds() {
        assert_eq!(
            VersionPolicy::capability_first().assess("9.0.0").unwrap().1,
            VersionSupport::Untested
        );
    }

    #[test]
    fn v0_1_policy_has_ordered_evidence_backed_boundaries() {
        let policy = VersionPolicy::v0_1();

        assert_eq!(
            policy.assess("0.12.5").unwrap().1,
            VersionSupport::Incompatible
        );
        assert_eq!(
            policy.assess("0.12.6").unwrap().1,
            VersionSupport::Compatible
        );
        assert_eq!(
            policy.assess("0.32.5").unwrap().1,
            VersionSupport::Compatible
        );
        assert_eq!(
            policy.assess("0.12.6-rc.1").unwrap().1,
            VersionSupport::Untested
        );
        assert_eq!(
            policy.assess("0.32.5-rc.1").unwrap().1,
            VersionSupport::Untested
        );
        assert_eq!(policy.assess("0.32.6").unwrap().1, VersionSupport::Untested);
    }

    #[test]
    fn reversed_tested_range_is_rejected() {
        assert_eq!(
            VersionPolicy::with_tested_range(Version::new(0, 32, 5), Version::new(0, 12, 6)),
            Err(InvalidVersionRange)
        );
    }
}
