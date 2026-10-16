use std::sync::Arc;

use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SecureStoreError {
    #[error("operating system credential storage is unavailable")]
    Unavailable,
    #[error("access to operating system credential storage was denied")]
    AccessDenied,
    #[error("credential storage input is invalid")]
    InvalidInput,
}

pub trait SecureStore: Send + Sync {
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), SecureStoreError>;
    fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecureStoreError>;
    fn delete(&self, service: &str, account: &str) -> Result<(), SecureStoreError>;
}

/// Keyring boundary injectable without accessing actual OS credentials in tests.
pub trait CredentialBackend: Send + Sync {
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), keyring::Error>;
    fn get(&self, service: &str, account: &str) -> Result<String, keyring::Error>;
    fn delete(&self, service: &str, account: &str) -> Result<(), keyring::Error>;
}

struct OsKeyringBackend;

impl CredentialBackend for OsKeyringBackend {
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), keyring::Error> {
        keyring::Entry::new(service, account)?.set_password(secret)
    }

    fn get(&self, service: &str, account: &str) -> Result<String, keyring::Error> {
        keyring::Entry::new(service, account)?.get_password()
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), keyring::Error> {
        keyring::Entry::new(service, account)?.delete_credential()
    }
}

pub struct KeyringSecureStore {
    backend: Arc<dyn CredentialBackend>,
}

impl Default for KeyringSecureStore {
    fn default() -> Self {
        Self::with_backend(Arc::new(OsKeyringBackend))
    }
}

impl KeyringSecureStore {
    pub fn with_backend(backend: Arc<dyn CredentialBackend>) -> Self {
        Self { backend }
    }
}

impl SecureStore for KeyringSecureStore {
    fn set(&self, service: &str, account: &str, secret: &str) -> Result<(), SecureStoreError> {
        self.backend
            .set(service, account, secret)
            .map_err(map_error)
    }

    fn get(&self, service: &str, account: &str) -> Result<Option<String>, SecureStoreError> {
        match self.backend.get(service, account) {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(map_error(error)),
        }
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), SecureStoreError> {
        match self.backend.delete(service, account) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(map_error(error)),
        }
    }
}

/// Never preserve platform errors: their messages may contain credential data.
fn map_error(error: keyring::Error) -> SecureStoreError {
    match error {
        keyring::Error::NoStorageAccess(_) => SecureStoreError::AccessDenied,
        keyring::Error::TooLong(_, _) | keyring::Error::Invalid(_, _) => {
            SecureStoreError::InvalidInput
        }
        _ => SecureStoreError::Unavailable,
    }
}
