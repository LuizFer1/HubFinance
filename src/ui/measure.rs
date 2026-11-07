//! Largura estimada de texto na Inter Hub Medium, para escolher tamanho de fonte antes do
//! layout.
//!
//! O iced nao mede texto na hora de montar a tela (e o canvas nao mede nunca), entao a escolha
//! entre "cabe a 15" e "desce para 14" sai de uma tabela de avancos lida da propria fonte
//! (`fontTools`, `hmtx` de `assets/fonts/InterHub-Medium.ttf`, 2048 unidades por em). So os
//! caracteres de valores e enderecos estao na tabela; qualquer outro conta como o glifo mais
//! largo ("W"), para a estimativa errar sempre para o lado de sobrar espaco. O kerning (GPOS)
//! fica de fora: entre digitos com `tnum` congelado ele nao existe.

const UNITS_PER_EM: f32 = 2048.0;
/// "W", o glifo mais largo do alfabeto: teto para caractere fora da tabela.
const WIDEST: u16 = 2054;

fn advance(c: char) -> u16 {
    match c {
        // Digitos tabulares (`tnum` congelado) e os sinais que a fonte desenha na mesma largura.
        '0'..='9' | 'R' | '+' | '\u{2212}' | '-' => 1327,
        '$' => 1323,
        '.' | ',' | ':' | ' ' => 550,
        '/' => 757,
        'h' => 1232,
        'p' => 1266,
        's' => 1103,
        't' => 697,
        _ => WIDEST,
    }
}

/// Largura de `s` em px na Inter Hub Medium de `size` px.
pub fn medium_width(s: &str, size: f32) -> f32 {
    let units: u32 = s.chars().map(|c| u32::from(advance(c))).sum();
    units as f32 / UNITS_PER_EM * size
}

/// O maior tamanho de `steps` (do maior para o menor) em que `s` cabe em `available` px; se
/// nenhum couber, o menor.
pub fn fitting_size(s: &str, available: f32, steps: &[f32]) -> f32 {
    steps
        .iter()
        .copied()
        .find(|size| medium_width(s, *size) <= available)
        .or_else(|| steps.last().copied())
        .unwrap_or(12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn largura_casa_com_a_fonte() {
        // Valores medidos com fontTools na Inter Hub Medium.
        let url = medium_width("https://192.168.100.200:7777", 15.0);
        assert!((url - 223.3).abs() < 0.5, "{url}");
        let money = medium_width("R$ 9.999.999,99", 15.0);
        assert!((money - 123.0).abs() < 0.5, "{money}");
    }

    #[test]
    fn caractere_fora_da_tabela_conta_como_o_mais_largo() {
        assert!(medium_width("é", 10.0) >= medium_width("W", 10.0));
        assert!(medium_width("é", 10.0) > medium_width("0", 10.0));
    }

    #[test]
    fn tamanho_desce_ate_caber() {
        let steps = [15.0, 14.0, 13.0];
        assert_eq!(
            fitting_size("https://192.168.0.5:7777", 300.0, &steps),
            15.0
        );
        // 223 a 15, 208 a 14.
        assert_eq!(
            fitting_size("https://192.168.100.200:7777", 215.0, &steps),
            14.0
        );
        assert_eq!(
            fitting_size("https://192.168.100.200:7777", 50.0, &steps),
            13.0
        );
    }
}
