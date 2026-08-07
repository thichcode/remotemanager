pub mod dpapi;
pub mod net;

pub fn encrypt(plaintext: &str) -> Result<String, String> {
    dpapi::encrypt_data(plaintext)
}

pub fn decrypt(ciphertext: &str) -> Result<String, String> {
    dpapi::decrypt_data(ciphertext)
}
