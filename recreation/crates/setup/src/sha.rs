//! Incremental SHA-256 through Windows CNG (no additional crates).

use std::ptr::{null, null_mut};
use windows_sys::Win32::Security::Cryptography::{
    BCRYPT_ALG_HANDLE, BCRYPT_HASH_HANDLE, BCRYPT_SHA256_ALG_HANDLE, BCryptCreateHash,
    BCryptDestroyHash, BCryptFinishHash, BCryptHashData,
};

pub struct Sha256(BCRYPT_HASH_HANDLE);

impl Sha256 {
    pub fn new() -> Result<Self, i32> {
        let mut hash: BCRYPT_HASH_HANDLE = null_mut();
        // SAFETY: the pseudo-handle needs no provider; CNG allocates the object.
        let status = unsafe {
            BCryptCreateHash(
                BCRYPT_SHA256_ALG_HANDLE as BCRYPT_ALG_HANDLE,
                &mut hash,
                null_mut(),
                0,
                null(),
                0,
                0,
            )
        };
        if status < 0 || hash.is_null() {
            return Err(status);
        }
        Ok(Self(hash))
    }

    pub fn update(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            let chunk = bytes.len().min(1 << 30);
            // SAFETY: the handle is live and the slice holds `chunk` bytes.
            unsafe { BCryptHashData(self.0, bytes.as_ptr(), chunk as u32, 0) };
            bytes = &bytes[chunk..];
        }
    }

    pub fn finish(self) -> Result<[u8; 32], i32> {
        let mut digest = [0u8; 32];
        // SAFETY: the handle is live and the output holds 32 bytes.
        let status = unsafe { BCryptFinishHash(self.0, digest.as_mut_ptr(), 32, 0) };
        if status < 0 {
            return Err(status);
        }
        Ok(digest)
    }
}

impl Drop for Sha256 {
    fn drop(&mut self) {
        // SAFETY: created by BCryptCreateHash and destroyed exactly once.
        unsafe { BCryptDestroyHash(self.0) };
    }
}

pub fn hex(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn known_vector() {
        let mut hasher = super::Sha256::new().unwrap();
        hasher.update(b"abc");
        assert_eq!(
            super::hex(&hasher.finish().unwrap()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
