use crate::error::{AppError, AppResult};

const CREDENTIAL_PREFIX: &str = "DeskFlow AI/provider/";
const MAX_SECRET_BYTES: usize = 2_048;

fn target_name(provider_id: &str) -> AppResult<String> {
    if provider_id.is_empty()
        || provider_id.len() > 64
        || !provider_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(AppError::CredentialStorage(
            "the provider identifier is not allowlisted.".to_string(),
        ));
    }
    Ok(format!("{CREDENTIAL_PREFIX}{provider_id}"))
}

pub fn write(provider_id: &str, secret: &str) -> AppResult<()> {
    let trimmed = secret.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_SECRET_BYTES {
        return Err(AppError::CredentialStorage(format!(
            "API keys must contain between 1 and {MAX_SECRET_BYTES} bytes."
        )));
    }
    write_platform(&target_name(provider_id)?, trimmed.as_bytes())
}

pub fn read(provider_id: &str) -> AppResult<Option<String>> {
    let Some(bytes) = read_platform(&target_name(provider_id)?)? else {
        return Ok(None);
    };
    let secret = String::from_utf8(bytes).map_err(|_| {
        AppError::CredentialStorage(
            "the saved credential is not valid UTF-8; remove and add it again.".to_string(),
        )
    })?;
    if secret.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(secret))
}

pub fn exists(provider_id: &str) -> AppResult<bool> {
    exists_platform(&target_name(provider_id)?)
}

pub fn delete(provider_id: &str) -> AppResult<()> {
    delete_platform(&target_name(provider_id)?)
}

#[cfg(windows)]
fn write_platform(target: &str, secret: &[u8]) -> AppResult<()> {
    use windows::{
        Win32::Security::Credentials::{
            CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredWriteW,
        },
        core::PWSTR,
    };

    let mut target_wide = target.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let mut user_wide = "DeskFlow AI"
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut secret_bytes = secret.to_vec();
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target_wide.as_mut_ptr()),
        CredentialBlobSize: secret_bytes.len() as u32,
        CredentialBlob: secret_bytes.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: PWSTR(user_wide.as_mut_ptr()),
        ..Default::default()
    };

    // SAFETY: all pointers remain valid for the duration of the synchronous Win32 call.
    let result = unsafe { CredWriteW(&credential, 0) };
    secret_bytes.fill(0);
    result.map_err(|_| {
        AppError::CredentialStorage(
            "Windows Credential Manager could not save the API key.".to_string(),
        )
    })
}

#[cfg(windows)]
fn read_platform(target: &str) -> AppResult<Option<Vec<u8>>> {
    use std::{ffi::c_void, ptr};

    use windows::{
        Win32::{
            Foundation::ERROR_NOT_FOUND,
            Security::Credentials::{CRED_TYPE_GENERIC, CREDENTIALW, CredFree, CredReadW},
        },
        core::{HRESULT, PCWSTR},
    };

    let target_wide = target.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let mut raw: *mut CREDENTIALW = ptr::null_mut();
    // SAFETY: target_wide is null-terminated and raw is an out pointer owned by CredReadW.
    if let Err(error) = unsafe {
        CredReadW(
            PCWSTR(target_wide.as_ptr()),
            CRED_TYPE_GENERIC,
            None,
            &mut raw,
        )
    } {
        if error.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0) {
            return Ok(None);
        }
        return Err(AppError::CredentialStorage(
            "Windows Credential Manager could not read the API key.".to_string(),
        ));
    }
    if raw.is_null() {
        return Err(AppError::CredentialStorage(
            "Windows Credential Manager returned an invalid credential.".to_string(),
        ));
    }

    // SAFETY: CredReadW returned a live CREDENTIALW and blob for the documented size.
    let bytes = unsafe {
        let credential = &*raw;
        let value = std::slice::from_raw_parts(
            credential.CredentialBlob,
            credential.CredentialBlobSize as usize,
        )
        .to_vec();
        CredFree(raw.cast::<c_void>());
        value
    };
    Ok(Some(bytes))
}

#[cfg(windows)]
fn delete_platform(target: &str) -> AppResult<()> {
    use windows::{
        Win32::{
            Foundation::ERROR_NOT_FOUND,
            Security::Credentials::{CRED_TYPE_GENERIC, CredDeleteW},
        },
        core::{HRESULT, PCWSTR},
    };

    let target_wide = target.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    // SAFETY: target_wide is null-terminated and remains live for the synchronous call.
    match unsafe { CredDeleteW(PCWSTR(target_wide.as_ptr()), CRED_TYPE_GENERIC, None) } {
        Ok(()) => Ok(()),
        Err(error) if error.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0) => Ok(()),
        Err(_) => Err(AppError::CredentialStorage(
            "Windows Credential Manager could not remove the API key.".to_string(),
        )),
    }
}

#[cfg(windows)]
fn exists_platform(target: &str) -> AppResult<bool> {
    use std::{ffi::c_void, ptr};

    use windows::{
        Win32::{
            Foundation::ERROR_NOT_FOUND,
            Security::Credentials::{CRED_TYPE_GENERIC, CREDENTIALW, CredFree, CredReadW},
        },
        core::{HRESULT, PCWSTR},
    };

    let target_wide = target.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let mut raw: *mut CREDENTIALW = ptr::null_mut();
    // SAFETY: target_wide is null-terminated and raw is an out pointer owned by CredReadW.
    match unsafe {
        CredReadW(
            PCWSTR(target_wide.as_ptr()),
            CRED_TYPE_GENERIC,
            None,
            &mut raw,
        )
    } {
        Ok(()) if raw.is_null() => Err(AppError::CredentialStorage(
            "Windows Credential Manager returned an invalid credential.".to_string(),
        )),
        Ok(()) => {
            // SAFETY: CredReadW returned this allocation and CredFree is its required release.
            unsafe { CredFree(raw.cast::<c_void>()) };
            Ok(true)
        }
        Err(error) if error.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0) => Ok(false),
        Err(_) => Err(AppError::CredentialStorage(
            "Windows Credential Manager could not inspect the API key.".to_string(),
        )),
    }
}

#[cfg(not(windows))]
fn write_platform(_target: &str, _secret: &[u8]) -> AppResult<()> {
    Err(AppError::CredentialStorage(
        "secure API-key storage is available only on Windows.".to_string(),
    ))
}

#[cfg(not(windows))]
fn read_platform(_target: &str) -> AppResult<Option<Vec<u8>>> {
    Ok(None)
}

#[cfg(not(windows))]
fn delete_platform(_target: &str) -> AppResult<()> {
    Err(AppError::CredentialStorage(
        "secure API-key storage is available only on Windows.".to_string(),
    ))
}

#[cfg(not(windows))]
fn exists_platform(_target: &str) -> AppResult<bool> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_names_are_bounded_and_allowlisted() {
        assert_eq!(
            target_name("opencode_zen").expect("allowlisted provider"),
            "DeskFlow AI/provider/opencode_zen"
        );
        assert!(target_name("../other").is_err());
        assert!(target_name("").is_err());
    }

    #[test]
    fn empty_secrets_are_rejected_before_platform_storage() {
        assert!(write("openai", "   ").is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_credential_manager_round_trip() {
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = delete(&self.0);
            }
        }

        let provider = format!("credential_probe_{}", std::process::id());
        let _cleanup = Cleanup(provider.clone());
        let _ = delete(&provider);
        write(&provider, "temporary-test-secret").expect("write credential");
        assert_eq!(
            read(&provider).expect("read credential").as_deref(),
            Some("temporary-test-secret")
        );
        delete(&provider).expect("delete credential");
        assert_eq!(read(&provider).expect("confirm deletion"), None);
    }
}
