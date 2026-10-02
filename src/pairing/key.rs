//! Chave de longa duracao do aparelho e o hash que o hub guarda dela.

use sha2::{Digest, Sha256};

/// 32 bytes de `rand::fill` em hex. Hex, e nao base64, para nao trazer mais um crate.
pub fn generate_key() -> String {
    let mut bytes = [0u8; 32];
    rand::fill(&mut bytes);
    hex::encode(bytes)
}

/// O banco guarda so isto: quem copiar `hub.sqlite` nao consegue se passar por um aparelho.
pub fn hash_key(key: &str) -> String {
    hex::encode(Sha256::digest(key.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chave_tem_64_hex_minusculos_e_e_aleatoria() {
        let key = generate_key();
        assert_eq!(key.len(), 64);
        assert!(
            key.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "{key}"
        );
        assert_ne!(generate_key(), key);
    }

    #[test]
    fn hash_e_sha256_hex_estavel() {
        assert_eq!(
            hash_key("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(hash_key("abc"), hash_key("abc"));
    }
}
