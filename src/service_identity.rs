use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{
    Signer, SigningKey, VerifyingKey,
    pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey},
};
use keyring::{Entry, Error as KeyringError};
use sha2::{Digest, Sha256};

const CREDENTIAL_VERSION: &str = "ed25519-v1:";

#[derive(Debug, Clone)]
pub struct ServiceIdentityOptions {
    pub service: String,
    pub account: String,
    pub linux_fallback_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LegacyServiceIdentity {
    pub key_id: Option<String>,
    pub public_key: String,
    pub private_key: String,
}

#[derive(Debug, Clone)]
pub struct ServiceIdentityInfo {
    pub key_id: String,
    pub public_key: String,
    pub backend: String,
}

pub struct ServiceIdentity {
    signing_key: SigningKey,
    info: ServiceIdentityInfo,
}

impl ServiceIdentity {
    pub fn info(&self) -> ServiceIdentityInfo {
        self.info.clone()
    }

    pub fn sign(&self, data: &[u8]) -> String {
        STANDARD.encode(self.signing_key.sign(data).to_bytes())
    }
}

fn validate_options(options: &ServiceIdentityOptions) -> Result<()> {
    if options.service.trim().is_empty() || options.account.trim().is_empty() {
        return Err(anyhow!("Service identity namespace is empty"));
    }
    if options.service.contains('\0') || options.account.contains('\0') {
        return Err(anyhow!(
            "Service identity namespace contains NUL characters"
        ));
    }
    if let Some(path) = &options.linux_fallback_path
        && !Path::new(path).is_absolute()
    {
        return Err(anyhow!("Service identity fallback path must be absolute"));
    }
    Ok(())
}

fn public_identity(signing_key: &SigningKey, backend: &str) -> Result<ServiceIdentityInfo> {
    let public_der = signing_key
        .verifying_key()
        .to_public_key_der()
        .context("Unable to encode service identity public key")?;
    let public_bytes = public_der.as_bytes();
    let key_id = Sha256::digest(public_bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(ServiceIdentityInfo {
        key_id,
        public_key: STANDARD.encode(public_bytes),
        backend: backend.to_string(),
    })
}

fn decode_legacy(legacy: LegacyServiceIdentity) -> Result<SigningKey> {
    let signing_key = SigningKey::from_pkcs8_pem(legacy.private_key.trim())
        .context("Invalid legacy service identity private key")?;
    let public_der = STANDARD
        .decode(legacy.public_key.split_whitespace().collect::<String>())
        .context("Invalid legacy service identity public key")?;
    let verifying_key = VerifyingKey::from_public_key_der(&public_der)
        .context("Invalid legacy service identity public key")?;
    if signing_key.verifying_key() != verifying_key {
        return Err(anyhow!("Legacy service identity key pair does not match"));
    }
    let info = public_identity(&signing_key, "migration")?;
    if legacy
        .key_id
        .is_some_and(|key_id| key_id.trim() != info.key_id)
    {
        return Err(anyhow!("Legacy service identity key ID does not match"));
    }
    Ok(signing_key)
}

fn generate_signing_key() -> Result<SigningKey> {
    let mut seed = [0_u8; 32];
    getrandom::fill(&mut seed)
        .map_err(|error| anyhow!("Unable to generate service identity: {error}"))?;
    Ok(SigningKey::from_bytes(&seed))
}

fn imported_or_generated(legacy: Option<LegacyServiceIdentity>) -> Result<SigningKey> {
    match legacy {
        Some(legacy) => decode_legacy(legacy),
        None => generate_signing_key(),
    }
}

fn encode_credential(signing_key: &SigningKey) -> Result<String> {
    let document = signing_key
        .to_pkcs8_der()
        .context("Unable to encode service identity")?;
    Ok(format!(
        "{CREDENTIAL_VERSION}{}",
        STANDARD.encode(document.as_bytes())
    ))
}

fn decode_credential(value: &str) -> Result<SigningKey> {
    let encoded = value
        .strip_prefix(CREDENTIAL_VERSION)
        .ok_or_else(|| anyhow!("Unsupported service identity credential version"))?;
    let document = STANDARD
        .decode(encoded)
        .context("Invalid service identity credential")?;
    SigningKey::from_pkcs8_der(&document).context("Invalid service identity credential")
}

fn keyring_backend() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows-credential-manager"
    } else if cfg!(target_os = "macos") {
        "macos-keychain"
    } else {
        "linux-secret-service"
    }
}

#[cfg(target_os = "linux")]
fn fallback_path(options: &ServiceIdentityOptions) -> Result<PathBuf> {
    options
        .linux_fallback_path
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| {
            anyhow!("Linux Secret Service is unavailable and no fallback path was supplied")
        })
}

#[cfg(target_os = "linux")]
fn read_fallback(options: &ServiceIdentityOptions) -> Result<Option<SigningKey>> {
    use std::fs;
    let Some(path) = options.linux_fallback_path.as_ref().map(PathBuf::from) else {
        return Ok(None);
    };
    match fs::read_to_string(path) {
        Ok(value) => Ok(Some(decode_credential(value.trim())?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[cfg(target_os = "linux")]
fn write_fallback(options: &ServiceIdentityOptions, signing_key: &SigningKey) -> Result<()> {
    use std::{
        fs,
        os::unix::fs::{OpenOptionsExt, PermissionsExt},
    };
    let path = fallback_path(options)?;
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Invalid fallback path"))?;
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&temporary)?
        .write_all(encode_credential(signing_key)?.as_bytes())?;
    fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(target_os = "linux")]
use std::io::Write;

pub fn open_service_identity(
    options: &ServiceIdentityOptions,
    legacy: Option<LegacyServiceIdentity>,
) -> Result<ServiceIdentity> {
    validate_options(options)?;
    let entry = Entry::new(options.service.trim(), options.account.trim())?;
    let keyring_result = entry.get_password();
    let (signing_key, backend) = match keyring_result {
        Ok(value) => {
            let stored = decode_credential(&value)?;
            let selected = match legacy {
                Some(legacy) => {
                    let imported = decode_legacy(legacy)?;
                    if imported.verifying_key() != stored.verifying_key() {
                        entry.set_password(&encode_credential(&imported)?)?;
                        imported
                    } else {
                        stored
                    }
                }
                None => stored,
            };
            (selected, keyring_backend())
        }
        Err(KeyringError::NoEntry) => {
            #[cfg(target_os = "linux")]
            let key = match read_fallback(options)? {
                Some(key) => key,
                None => imported_or_generated(legacy)?,
            };
            #[cfg(not(target_os = "linux"))]
            let key = imported_or_generated(legacy)?;
            entry.set_password(&encode_credential(&key)?)?;
            #[cfg(target_os = "linux")]
            remove_fallback(options)?;
            (key, keyring_backend())
        }
        #[cfg(target_os = "linux")]
        Err(_) => {
            let key = match read_fallback(options)? {
                Some(key) => key,
                None => {
                    let key = imported_or_generated(legacy)?;
                    write_fallback(options, &key)?;
                    key
                }
            };
            (key, "linux-protected-file")
        }
        #[cfg(not(target_os = "linux"))]
        Err(error) => return Err(error.into()),
    };
    let info = public_identity(&signing_key, backend)?;
    Ok(ServiceIdentity { signing_key, info })
}

pub fn delete_service_identity(options: &ServiceIdentityOptions) -> Result<()> {
    validate_options(options)?;
    let entry = Entry::new(options.service.trim(), options.account.trim())?;
    match entry.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => {}
        #[cfg(target_os = "linux")]
        Err(_) => {
            let path = fallback_path(options)?;
            if path.exists() {
                std::fs::remove_file(path)?;
            }
        }
        #[cfg(not(target_os = "linux"))]
        Err(error) => return Err(error.into()),
    }
    #[cfg(target_os = "linux")]
    remove_fallback(options)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn remove_fallback(options: &ServiceIdentityOptions) -> Result<()> {
    if let Some(path) = &options.linux_fallback_path {
        let path = Path::new(path);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, Verifier};

    #[test]
    fn generated_identity_signatures_verify() {
        let signing_key = generate_signing_key().unwrap();
        let identity = ServiceIdentity {
            info: public_identity(&signing_key, "test").unwrap(),
            signing_key,
        };
        let signature =
            Signature::from_slice(&STANDARD.decode(identity.sign(b"request")).unwrap()).unwrap();
        identity
            .signing_key
            .verifying_key()
            .verify(b"request", &signature)
            .unwrap();
    }

    #[test]
    fn credential_round_trip_preserves_identity() {
        let original = generate_signing_key().unwrap();
        let restored = decode_credential(&encode_credential(&original).unwrap()).unwrap();
        assert_eq!(original.verifying_key(), restored.verifying_key());
    }
}
