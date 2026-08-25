//! Cached Passport JWT signing and verification keys.

#![allow(dead_code)]

use std::collections::BTreeMap;

use jsonwebtoken::{DecodingKey, EncodingKey};

use super::config::{ConfiguredKey, PassportKeyMode, decoding_key, encoding_key};
use super::{JwtAlgorithm, JwtError, JwtErrorKind, JwtResult, PassportConfig};

pub(crate) struct KeyRing {
    mode: KeyRingMode,
}

enum KeyRingMode {
    Simple {
        algorithm: JwtAlgorithm,
        encoding_key: EncodingKey,
        decoding_key: DecodingKey,
    },
    Named {
        active_key: String,
        keys: BTreeMap<String, CachedKey>,
    },
}

struct CachedKey {
    algorithm: JwtAlgorithm,
    encoding_key: Option<EncodingKey>,
    decoding_key: DecodingKey,
}

impl KeyRing {
    pub(crate) fn from_config(config: &PassportConfig) -> JwtResult<Self> {
        let mode = match config.key_mode() {
            PassportKeyMode::SimpleHmac { secret } => {
                let algorithm = *config
                    .algorithms()
                    .first()
                    .ok_or_else(|| JwtError::new(JwtErrorKind::UnavailableSigningKey))?;
                KeyRingMode::Simple {
                    algorithm,
                    encoding_key: EncodingKey::from_secret(secret),
                    decoding_key: DecodingKey::from_secret(secret),
                }
            }
            PassportKeyMode::Named { active_key, keys } => {
                let mut cached = BTreeMap::new();
                for (key_id, key) in keys {
                    cached.insert(key_id.clone(), CachedKey::from_configured(key)?);
                }
                KeyRingMode::Named {
                    active_key: active_key.clone(),
                    keys: cached,
                }
            }
        };
        Ok(Self { mode })
    }

    pub(crate) fn active_signer(&self) -> JwtResult<(&EncodingKey, JwtAlgorithm, Option<&str>)> {
        match &self.mode {
            KeyRingMode::Simple {
                algorithm,
                encoding_key,
                ..
            } => Ok((encoding_key, *algorithm, None)),
            KeyRingMode::Named { active_key, keys } => {
                let key = keys
                    .get(active_key)
                    .ok_or_else(|| JwtError::new(JwtErrorKind::UnavailableSigningKey))?;
                let encoding_key = key
                    .encoding_key
                    .as_ref()
                    .ok_or_else(|| JwtError::new(JwtErrorKind::UnavailableSigningKey))?;
                Ok((encoding_key, key.algorithm, Some(active_key)))
            }
        }
    }

    pub(crate) fn verifier(&self, key_id: Option<&str>) -> JwtResult<(&DecodingKey, JwtAlgorithm)> {
        match &self.mode {
            KeyRingMode::Simple {
                algorithm,
                decoding_key,
                ..
            } => Ok((decoding_key, *algorithm)),
            KeyRingMode::Named { keys, .. } => {
                let key_id = key_id.ok_or_else(|| JwtError::new(JwtErrorKind::MissingKeyId))?;
                let key = keys
                    .get(key_id)
                    .ok_or_else(|| JwtError::new(JwtErrorKind::UnknownKeyId))?;
                Ok((&key.decoding_key, key.algorithm))
            }
        }
    }
}

impl CachedKey {
    fn from_configured(configured: &ConfiguredKey) -> JwtResult<Self> {
        match configured.secret.as_deref() {
            Some(secret) => Ok(Self {
                algorithm: configured.algorithm,
                encoding_key: Some(EncodingKey::from_secret(secret)),
                decoding_key: DecodingKey::from_secret(secret),
            }),
            None => {
                let public_material = configured
                    .public_key
                    .as_ref()
                    .ok_or_else(|| JwtError::new(JwtErrorKind::UnavailableSigningKey))?;
                let public_bytes = public_material.read()?;
                let decoding_key = decoding_key(configured.algorithm, &public_bytes)?;
                let encoding_key = configured
                    .private_key
                    .as_ref()
                    .map(|material| {
                        material
                            .read()
                            .and_then(|bytes| encoding_key(configured.algorithm, &bytes))
                    })
                    .transpose()?;
                Ok(Self {
                    algorithm: configured.algorithm,
                    encoding_key,
                    decoding_key,
                })
            }
        }
    }
}
