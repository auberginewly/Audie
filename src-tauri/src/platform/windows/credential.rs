use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows_sys::Win32::Foundation::ERROR_NOT_FOUND;
use windows_sys::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE,
    CRED_TYPE_GENERIC,
};

use crate::error::{AppError, AppResult};

const CREDENTIAL_SERVICE: &str = "com.audie.app.secure-storage";

pub(super) fn store_secret(key: &str, value: &str) -> AppResult<()> {
    let target = wide_null(&target_name(key));
    let mut secret = value.as_bytes().to_vec();
    let blob_size = u32::try_from(secret.len())
        .map_err(|_| AppError::Provider("secret value is too large".into()))?;
    let credential = CREDENTIALW {
        Flags: 0,
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_ptr().cast_mut(),
        Comment: std::ptr::null_mut(),
        LastWritten: Default::default(),
        CredentialBlobSize: blob_size,
        CredentialBlob: secret.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        AttributeCount: 0,
        Attributes: std::ptr::null_mut(),
        TargetAlias: std::ptr::null_mut(),
        UserName: std::ptr::null_mut(),
    };
    // SAFETY: Category 8 — FFI boundary. `credential` points to initialized fields;
    // target is NUL-terminated UTF-16, and CredentialBlob remains alive for call.
    let ok = unsafe { CredWriteW(&credential, 0) };
    if ok == 0 {
        Err(AppError::Internal(format!(
            "write Windows credential: {}",
            std::io::Error::last_os_error()
        )))
    } else {
        Ok(())
    }
}

pub(super) fn has_secret(key: &str) -> AppResult<bool> {
    match read_credential(key) {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(err) => Err(err),
    }
}

pub(super) fn read_secret(key: &str) -> AppResult<String> {
    match read_credential(key)? {
        Some(value) => Ok(value),
        None => Err(AppError::Provider("secret not found".into())),
    }
}

pub(super) fn delete_secret(key: &str) -> AppResult<()> {
    let target = wide_null(&target_name(key));
    // SAFETY: Category 8 — FFI boundary. `target` is a valid, NUL-terminated
    // UTF-16 string and no output pointers are involved.
    let ok = unsafe { CredDeleteW(target.as_ptr(), CRED_TYPE_GENERIC, 0) };
    if ok != 0 || last_error_code() == ERROR_NOT_FOUND {
        Ok(())
    } else {
        Err(AppError::Internal(format!(
            "delete Windows credential: {}",
            std::io::Error::last_os_error()
        )))
    }
}

fn read_credential(key: &str) -> AppResult<Option<String>> {
    let target = wide_null(&target_name(key));
    let mut credential = std::ptr::null_mut();
    // SAFETY: Category 8 — FFI boundary. `target` is NUL-terminated UTF-16 and
    // `credential` is a valid out-pointer; Windows owns returned memory.
    let ok = unsafe { CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) };
    if ok == 0 {
        if last_error_code() == ERROR_NOT_FOUND {
            return Ok(None);
        }
        return Err(AppError::Internal(format!(
            "read Windows credential: {}",
            std::io::Error::last_os_error()
        )));
    }
    if credential.is_null() {
        return Err(AppError::Internal(
            "read Windows credential returned null".into(),
        ));
    }
    let _guard = CredentialGuard(credential.cast());
    // SAFETY: Category 8 — FFI boundary. `credential` is non-null and valid until
    // `_guard` drops; fields are initialized by CredReadW for this credential.
    let cred = unsafe { &*credential };
    let len = usize::try_from(cred.CredentialBlobSize)
        .map_err(|_| AppError::Internal("credential blob size overflow".into()))?;
    if cred.CredentialBlob.is_null() {
        return Err(AppError::Internal("credential blob is null".into()));
    }
    // SAFETY: Category 8 — FFI boundary. CredentialBlob points to
    // CredentialBlobSize bytes owned by Windows and valid until CredFree.
    let bytes = unsafe { std::slice::from_raw_parts(cred.CredentialBlob, len) };
    String::from_utf8(bytes.to_vec())
        .map(Some)
        .map_err(|_| AppError::Internal("Windows credential secret is not UTF-8".into()))
}

struct CredentialGuard(*mut std::ffi::c_void);

impl Drop for CredentialGuard {
    fn drop(&mut self) {
        // SAFETY: Category 12 — invalid free/double free. This guard is created
        // exactly once for each successful CredReadW pointer and owns the CredFree.
        unsafe {
            CredFree(self.0);
        }
    }
}

fn target_name(key: &str) -> String {
    format!("{CREDENTIAL_SERVICE}:{key}")
}

fn wide_null(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain([0]).collect()
}

fn last_error_code() -> u32 {
    std::io::Error::last_os_error()
        .raw_os_error()
        .and_then(|code| u32::try_from(code).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_name_namespaces_key_under_audie_service() {
        assert_eq!(
            target_name("openai_api_key"),
            "com.audie.app.secure-storage:openai_api_key"
        );
    }
}
