//! HLC no formato do app: `millis(13)-counter(4 hex maiusculo)-deviceId(26 Crockford)`.

use std::cmp::Ordering;

use super::ids::is_crockford_upper;

pub const HLC_LEN: usize = 13 + 1 + 4 + 1 + 26;

/// `millis(13)-counter(4 hex maiusculo)-deviceId(26 Crockford)`. Sem regex: tres fatias de
/// largura fixa. A largura fixa e o que faz `compare_hlc` ser so comparacao de string.
pub fn is_valid_hlc(value: &str) -> bool {
    // Trabalha em bytes: todos os caracteres validos sao ASCII, entao qualquer byte fora do
    // esperado (inclusive o primeiro de um caractere multibyte) ja reprova.
    let bytes = value.as_bytes();
    if bytes.len() != HLC_LEN {
        return false;
    }
    let (millis, rest) = bytes.split_at(13);
    let (dash1, rest) = rest.split_at(1);
    let (counter, rest) = rest.split_at(4);
    let (dash2, device) = rest.split_at(1);
    millis.iter().all(u8::is_ascii_digit)
        && dash1 == b"-"
        && counter
            .iter()
            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(b))
        && dash2 == b"-"
        && device.iter().all(|b| is_crockford_upper(*b))
}

/// Comparacao de string pura. So e correta porque `is_valid_hlc` garantiu a largura fixa;
/// parsear para comparar abriria espaco para divergir do `compareHlc` do app.
pub fn compare_hlc(a: &str, b: &str) -> Ordering {
    a.cmp(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEVICE: &str = "01HZZZZZZZZZZZZZZZZZZZZZZA";

    fn hlc(millis: &str, counter: &str, device: &str) -> String {
        format!("{millis}-{counter}-{device}")
    }

    #[test]
    fn aceita_hlc_bem_formado() {
        assert!(is_valid_hlc(
            "1759344000000-0000-01HZZZZZZZZZZZZZZZZZZZZZZA"
        ));
        assert!(is_valid_hlc(
            "0000000000000-FFFF-00000000000000000000000000"
        ));
        assert_eq!(
            HLC_LEN,
            "1759344000000-0000-01HZZZZZZZZZZZZZZZZZZZZZZA".len()
        );
    }

    #[test]
    fn rejeita_millis_com_largura_errada() {
        assert!(!is_valid_hlc(&hlc("175934400000", "0000", DEVICE)));
        assert!(!is_valid_hlc(&hlc("17593440000000", "0000", DEVICE)));
        assert!(!is_valid_hlc(&hlc("17593440000a0", "0000", DEVICE)));
    }

    #[test]
    fn rejeita_counter_minusculo_ou_largura_errada() {
        assert!(!is_valid_hlc(&hlc("1759344000000", "00ff", DEVICE)));
        assert!(!is_valid_hlc(&hlc("1759344000000", "000", DEVICE)));
        assert!(!is_valid_hlc(&hlc("1759344000000", "00000", DEVICE)));
        assert!(!is_valid_hlc(&hlc("1759344000000", "00G0", DEVICE)));
    }

    #[test]
    fn rejeita_device_fora_do_alfabeto_ou_largura_errada() {
        for bad in ['I', 'L', 'O', 'U', 'a'] {
            let device = format!("01HZZZZZZZZZZZZZZZZZZZZZZ{bad}");
            assert!(
                !is_valid_hlc(&hlc("1759344000000", "0000", &device)),
                "{bad}"
            );
        }
        assert!(!is_valid_hlc(&hlc("1759344000000", "0000", &DEVICE[..25])));
        assert!(!is_valid_hlc(&hlc(
            "1759344000000",
            "0000",
            &format!("{DEVICE}A")
        )));
    }

    #[test]
    fn rejeita_separador_vazio_e_espaco() {
        assert!(!is_valid_hlc(
            "1759344000000_0000_01HZZZZZZZZZZZZZZZZZZZZZZA"
        ));
        assert!(!is_valid_hlc(
            "1759344000000-0000_01HZZZZZZZZZZZZZZZZZZZZZZA"
        ));
        assert!(!is_valid_hlc(""));
        assert!(!is_valid_hlc(
            "1759344000000-0000-01HZZZZZZZZZZZZZZZZZZZZZZA "
        ));
        // Multibyte com a mesma contagem de bytes nao pode passar por fatiamento de string.
        assert!(!is_valid_hlc(
            "175934400000é-000-01HZZZZZZZZZZZZZZZZZZZZZZA"
        ));
    }

    #[test]
    fn largura_fixa_torna_lexicografico_igual_a_numerico() {
        let a = hlc("0000000000999", "0000", DEVICE);
        let b = hlc("0000000001000", "0000", DEVICE);
        assert_eq!(compare_hlc(&a, &b), Ordering::Less);
        assert_eq!(compare_hlc(&b, &a), Ordering::Greater);
    }

    #[test]
    fn counter_desempata_mesmo_millis() {
        let a = hlc("1759344000000", "0001", DEVICE);
        let b = hlc("1759344000000", "0010", DEVICE);
        assert_eq!(compare_hlc(&a, &b), Ordering::Less);
        assert_eq!(compare_hlc(&a, &a), Ordering::Equal);
    }
}
