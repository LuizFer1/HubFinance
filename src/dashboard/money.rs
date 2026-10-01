//! Moeda em pt-BR: `R$ 1.234,56`, sempre a partir de centavos inteiros.
//!
//! O sinal de negativo e U+2212 (`−`), nao o hifen: e o que `features/ui/money.tsx` do app e o
//! README do handoff usam, e o hifen fica mais curto que o `+` ao lado, desalinhando colunas.
//! O espaco depois de `R$` e o comum, nao o inseparavel do `Intl`: o iced desenha os dois
//! iguais e o teste fica legivel.

/// Sinal de menos tipografico (U+2212).
pub const MINUS: &str = "\u{2212}";

/// Valor grande dos cards em tres pedacos: sinal e `R$` pequenos, inteiro grande, centavos
/// pequenos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoneyParts {
    pub sign: &'static str,
    pub whole: String,
    pub cents: String,
}

/// `1234567` -> `"1.234.567"`.
pub fn group_thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push('.');
        }
        out.push(c);
    }
    out
}

/// Valor absoluto: quem chama decide o sinal (`−` na despesa, `+` na receita).
pub fn format_brl(minor: i64) -> String {
    let parts = money_parts(minor);
    format!("R$ {},{}", parts.whole, parts.cents)
}

/// `−` so quando negativo (saldo, "Sobrou ...").
pub fn signed_brl(minor: i64) -> String {
    let sign = if minor < 0 { MINUS } else { "" };
    format!("{sign}{}", format_brl(minor))
}

/// `+` quando >= 0, `−` quando negativo: saldo do dia na lista. O zero leva `+`, como o
/// `(net >= 0 ? '+' : '')` do prototipo.
pub fn signed_brl_plus(minor: i64) -> String {
    let sign = if minor < 0 { MINUS } else { "+" };
    format!("{sign}{}", format_brl(minor))
}

pub fn money_parts(minor: i64) -> MoneyParts {
    // `unsigned_abs` nao estoura em `i64::MIN`, ao contrario de `abs`.
    let abs = minor.unsigned_abs();
    MoneyParts {
        sign: if minor < 0 { MINUS } else { "" },
        whole: group_thousands(abs / 100),
        cents: format!("{:02}", abs % 100),
    }
}

/// Rotulo do eixo do grafico: "R$ 8 mil", "R$ 7,5 mil", "R$ 500". Uma casa decimal, sem zero a
/// direita, como o `toLocaleString('pt-BR')` do prototipo faz com os tetos que ele gera.
pub fn axis_label(minor: i64) -> String {
    let abs = minor.unsigned_abs();
    let reais = abs / 100;
    if reais < 1000 {
        return format!("R$ {reais}");
    }
    // Decimos de milhar, arredondados: 1 mil = 100_000 centavos, 0,1 mil = 10_000.
    let tenths = (abs + 5_000) / 10_000;
    let (int, frac) = (tenths / 10, tenths % 10);
    if frac == 0 {
        format!("R$ {} mil", group_thousands(int))
    } else {
        format!("R$ {},{frac} mil", group_thousands(int))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formata_absoluto() {
        assert_eq!(format_brl(0), "R$ 0,00");
        assert_eq!(format_brl(1), "R$ 0,01");
        assert_eq!(format_brl(1234), "R$ 12,34");
        assert_eq!(format_brl(123_456), "R$ 1.234,56");
        assert_eq!(format_brl(123_456_789), "R$ 1.234.567,89");
        // Sem sinal: e valor absoluto.
        assert_eq!(format_brl(-5), "R$ 0,05");
        // Nao estoura no extremo.
        assert!(format_brl(i64::MIN).starts_with("R$ 92.233.720.368.547.758,08"));
    }

    #[test]
    fn com_sinal() {
        assert_eq!(signed_brl(-5000), "\u{2212}R$ 50,00");
        assert_eq!(signed_brl(5000), "R$ 50,00");
        assert_eq!(signed_brl(0), "R$ 0,00");
        assert_eq!(signed_brl_plus(650_000), "+R$ 6.500,00");
        assert_eq!(signed_brl_plus(-1), "\u{2212}R$ 0,01");
        assert_eq!(signed_brl_plus(0), "+R$ 0,00");
    }

    #[test]
    fn partes_do_valor_grande() {
        assert_eq!(
            money_parts(-123_456),
            MoneyParts {
                sign: "\u{2212}",
                whole: "1.234".into(),
                cents: "56".into()
            }
        );
        assert_eq!(
            money_parts(7),
            MoneyParts {
                sign: "",
                whole: "0".into(),
                cents: "07".into()
            }
        );
    }

    #[test]
    fn milhar() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1000), "1.000");
        assert_eq!(group_thousands(1_234_567), "1.234.567");
    }

    #[test]
    fn rotulos_do_eixo() {
        assert_eq!(axis_label(800_000), "R$ 8 mil");
        assert_eq!(axis_label(750_000), "R$ 7,5 mil");
        assert_eq!(axis_label(1_250_000), "R$ 12,5 mil");
        assert_eq!(axis_label(50_000), "R$ 500");
        assert_eq!(axis_label(0), "R$ 0");
        assert_eq!(axis_label(100_000), "R$ 1 mil");
        assert_eq!(axis_label(100_000_000), "R$ 1.000 mil");
    }
}
