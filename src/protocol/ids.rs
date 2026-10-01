//! Ids de linha e nomes de tabela.

pub const ID_LEN: usize = 26;
pub const MAX_TABLE_LEN: usize = 64;

/// Alfabeto Crockford maiusculo: `0-9`, `A-H`, `J`, `K`, `M`, `N`, `P-T`, `V-Z`.
pub fn is_crockford_upper(byte: u8) -> bool {
    matches!(byte, b'0'..=b'9' | b'A'..=b'H' | b'J' | b'K' | b'M' | b'N' | b'P'..=b'T' | b'V'..=b'Z')
}

/// 26 chars Crockford. NAO e ULID estrito: `stableEntityId` do app usa o alfabeto inteiro em
/// todas as posicoes, e exigir primeiro caractere <= `7` rejeitaria toda recorrencia.
pub fn is_valid_id(value: &str) -> bool {
    value.len() == ID_LEN && value.bytes().all(is_crockford_upper)
}

/// `^[A-Za-z][A-Za-z0-9_]{0,63}$`.
pub fn is_valid_table(value: &str) -> bool {
    let bytes = value.as_bytes();
    match bytes.split_first() {
        Some((first, rest)) => {
            bytes.len() <= MAX_TABLE_LEN
                && first.is_ascii_alphabetic()
                && rest.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_')
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aceita_id_deterministico_comecando_com_z() {
        assert!(is_valid_id("ZZZZZZZZZZZZZZZZZZZZZZZZZZ"));
        assert!(is_valid_id("01HZZZZZZZZZZZZZZZZZZZZZC1"));
        assert!(is_valid_id("0123456789ABCDEFGHJKMNPQRS"));
        assert!(is_valid_id("TVWXYZ00000000000000000000"));
    }

    #[test]
    fn rejeita_id_fora_do_alfabeto_ou_largura_errada() {
        for bad in ['I', 'L', 'O', 'U', 'a', 'z', '-'] {
            let id = format!("01HZZZZZZZZZZZZZZZZZZZZZZ{bad}");
            assert!(!is_valid_id(&id), "{bad}");
        }
        assert!(!is_valid_id(&"0".repeat(25)));
        assert!(!is_valid_id(&"0".repeat(27)));
        assert!(!is_valid_id(""));
    }

    #[test]
    fn aceita_nomes_de_tabela_do_app() {
        for name in [
            "transactions",
            "paymentMethods",
            "recurrence_adjustments",
            "a",
        ] {
            assert!(is_valid_table(name), "{name}");
        }
        assert!(is_valid_table(&format!("a{}", "b".repeat(63))));
    }

    #[test]
    fn rejeita_nomes_de_tabela_invalidos() {
        for name in [
            "",
            "1abc",
            "_abc",
            "pay-methods",
            "pay methods",
            "tabela\u{e9}",
        ] {
            assert!(!is_valid_table(name), "{name}");
        }
        assert!(!is_valid_table(&format!("a{}", "b".repeat(64))));
    }
}
