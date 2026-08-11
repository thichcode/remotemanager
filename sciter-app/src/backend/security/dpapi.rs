use base64::{Engine as _, engine::general_purpose};

#[allow(non_snake_case)]
#[repr(C)]
struct DataBlob {
    cbData: u32,
    pbData: *mut u8,
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn LocalFree(hMem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
}

#[cfg(windows)]
pub fn encrypt_data(plaintext: &str) -> Result<String, String> {
    use windows::Win32::Security::Cryptography::{
        CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData,
    };

    let bytes = plaintext.as_bytes();
    let input_blob = DataBlob {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output_blob = DataBlob {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    unsafe {
        CryptProtectData(
            &input_blob as *const _ as *const _,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output_blob as *mut _ as *mut _,
        ).map_err(|e| format!("DPAPI encrypt failed: {:?}", e))?;

        let data = std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize);
        let encoded = general_purpose::STANDARD.encode(data);
        // Free using LocalFree (required for memory allocated by CryptProtectData)
        if !output_blob.pbData.is_null() {
            LocalFree(output_blob.pbData as *mut _);
        }
        Ok(encoded)
    }
}

#[cfg(windows)]
pub fn decrypt_data(ciphertext: &str) -> Result<String, String> {
    use windows::Win32::Security::Cryptography::{
        CRYPTPROTECT_UI_FORBIDDEN, CryptUnprotectData,
    };

    let decoded = general_purpose::STANDARD
        .decode(ciphertext)
        .map_err(|e| format!("Base64 decode failed: {}", e))?;

    let input_blob = DataBlob {
        cbData: decoded.len() as u32,
        pbData: decoded.as_ptr() as *mut u8,
    };
    let mut output_blob = DataBlob {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    unsafe {
        CryptUnprotectData(
            &input_blob as *const _ as *const _,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output_blob as *mut _ as *mut _,
        ).map_err(|e| format!("DPAPI decrypt failed: {:?}", e))?;

        let data = std::slice::from_raw_parts(output_blob.pbData, output_blob.cbData as usize);
        let result = String::from_utf8_lossy(data).to_string();
        if !output_blob.pbData.is_null() {
            LocalFree(output_blob.pbData as *mut _);
        }
        Ok(result)
    }
}

#[cfg(not(windows))]
pub fn encrypt_data(plaintext: &str) -> Result<String, String> {
    Ok(general_purpose::STANDARD.encode(plaintext))
}

#[cfg(not(windows))]
pub fn decrypt_data(ciphertext: &str) -> Result<String, String> {
    let decoded = general_purpose::STANDARD
        .decode(ciphertext)
        .map_err(|e| format!("Decode failed: {}", e))?;
    Ok(String::from_utf8_lossy(&decoded).to_string())
}
