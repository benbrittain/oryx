use crate::error::OryxError;
use once_cell::sync::Lazy;
use regex::Regex;

static DIGEST_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new("([0-9a-f]+):([0-9]+)").expect("Failed to compile digest regex"));

#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Digest {
    hash: String,
    size_bytes: i64,
}

// Intentionally using getters so that Digest creation is forced through
// the validation logic in FromStr.
impl Digest {
    fn hash(&self) -> &str {
        &self.hash
    }

    fn size_bytes(&self) -> i64 {
        self.size_bytes
    }
}

impl std::fmt::Display for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.hash, self.size_bytes)
    }
}

impl Into<protos::re::Digest> for Digest {
    fn into(self) -> protos::re::Digest {
        protos::re::Digest {
            hash: self.hash,
            size_bytes: self.size_bytes,
        }
    }
}

impl From<protos::re::Digest> for Digest {
    fn from(d: protos::re::Digest) -> Self {
        Digest {
            hash: d.hash,
            size_bytes: d.size_bytes,
        }
    }
}

impl std::str::FromStr for Digest {
    type Err = OryxError;

    fn from_str(digest: &str) -> Result<Digest, Self::Err> {
        let matches = DIGEST_REGEX
            .captures(digest)
            .ok_or_else(|| OryxError::InvalidDigest(digest.to_string()))?;
        Ok(Digest {
            hash: matches[1].to_string(),
            size_bytes: matches[2]
                .parse::<i64>()
                .map_err(|_| OryxError::InvalidDigest(digest.to_string()))?,
            ..Default::default()
        })
    }
}
