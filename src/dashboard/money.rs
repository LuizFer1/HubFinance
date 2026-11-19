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

/// "R$ 2.650": reais inteiros, arredondados meio para cima, sem centavos (`brl0` do prototipo,
/// usado no ritmo das caixinhas e na projecao). Valor absoluto, como `format_brl`.
pub fn whole_brl(minor: i64) -> String {
    let abs = minor.unsigned_abs();
    format!(
        "R$ {}",
        group_thousands(abs / 100 + u64::from(abs % 100 >= 50))
    )
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

/// "Ocultar valores" (spec 7.4): o que aparece no lugar de todo valor em R$. O mesmo `MASK` de
/// `features/privacy/privacy.ts` do app, para os dois paineis esconderem igual.
pub const MASK: &str = "R$ •••••";
/// No eixo do grafico cabe pouco: so os pontos.
pub const AXIS_MASK: &str = "•••";
/// Inteiro do valor grande quando oculto (o `moneyParts` do app).
pub const MASK_DIGITS: &str = "•••••";

/// Valor grande partido como o design desenha: `R$` e centavos pequenos, inteiro grande. Os
/// centavos ja trazem a virgula (",59"), como o `moneyParts` do app: oculto, `cents` fica vazio
/// e nao sobra uma virgula solta ao lado da mascara.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BigMoney {
    /// "", "+" ou "−".
    pub sign: &'static str,
    pub currency: &'static str,
    pub whole: String,
    /// ",59"; vazio em reais inteiros ou oculto.
    pub cents: String,
}

impl BigMoney {
    /// O valor corrido ("R$ 386,59"): leitor de tela e testes.
    pub fn text(&self) -> String {
        format!(
            "{}{} {}{}",
            self.sign, self.currency, self.whole, self.cents
        )
    }
}

/// Formatador de dinheiro das telas novas: o unico caminho do valor ate o texto, como o `brl`
/// do app. Com `hidden`, todo valor em R$ vira a mascara; porcentagens, datas e contagens nao
/// passam por aqui e continuam visiveis. Formatar com as funcoes soltas numa tela vazaria o
/// numero por cima do botao de ocultar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Money {
    pub hidden: bool,
}

impl Money {
    pub fn new(hidden: bool) -> Self {
        Self { hidden }
    }

    fn or_mask(self, shown: impl FnOnce() -> String) -> String {
        if self.hidden {
            MASK.to_string()
        } else {
            shown()
        }
    }

    /// `format_brl` (absoluto, com centavos).
    pub fn brl(self, minor: i64) -> String {
        self.or_mask(|| format_brl(minor))
    }

    /// `signed_brl`: `−` so no negativo. Oculto nao mostra o sinal (como o `signedBRL` do app:
    /// o sinal ja diz de que lado o valor caiu).
    pub fn signed(self, minor: i64) -> String {
        self.or_mask(|| signed_brl(minor))
    }

    /// `signed_brl_plus`: `+` no zero e no positivo.
    pub fn signed_plus(self, minor: i64) -> String {
        self.or_mask(|| signed_brl_plus(minor))
    }

    /// `whole_brl`: reais inteiros, para estimativas.
    pub fn whole(self, minor: i64) -> String {
        self.or_mask(|| whole_brl(minor))
    }

    /// Reais inteiros com `−` so no negativo ("R$ 387 até agora" no historico).
    pub fn signed_whole(self, minor: i64) -> String {
        self.or_mask(|| {
            let sign = if minor < 0 { MINUS } else { "" };
            format!("{sign}{}", whole_brl(minor))
        })
    }

    /// `compactBRL` do app: sem ",00" quando redondo, com centavos quando nao ("R$ 200",
    /// "R$ 333,33"). Para valores exatos, onde arredondar mostraria o que nao foi movido.
    pub fn compact(self, minor: i64) -> String {
        if minor % 100 == 0 {
            self.whole(minor)
        } else {
            self.brl(minor)
        }
    }

    /// Rotulo do eixo ("R$ 5 mil").
    pub fn axis(self, minor: i64) -> String {
        if self.hidden {
            AXIS_MASK.to_string()
        } else {
            axis_label(minor)
        }
    }

    /// Valor grande com centavos ("R$" "386" ",59"), sinal so no negativo.
    pub fn big(self, minor: i64) -> BigMoney {
        if self.hidden {
            return BigMoney {
                sign: "",
                currency: "R$",
                whole: MASK_DIGITS.to_string(),
                cents: String::new(),
            };
        }
        let parts = money_parts(minor);
        BigMoney {
            sign: parts.sign,
            currency: "R$",
            whole: parts.whole,
            cents: format!(",{}", parts.cents),
        }
    }

    /// Valor grande em reais inteiros ("R$" "2.624"), meio para cima como `whole_brl`.
    pub fn big_whole(self, minor: i64) -> BigMoney {
        if self.hidden {
            return self.big(minor);
        }
        let abs = minor.unsigned_abs();
        BigMoney {
            sign: if minor < 0 { MINUS } else { "" },
            currency: "R$",
            whole: group_thousands(abs / 100 + u64::from(abs % 100 >= 50)),
            cents: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatador_visivel_repete_as_funcoes_soltas() {
        let m = Money::new(false);
        assert_eq!(m.brl(38_659), "R$ 386,59");
        assert_eq!(m.signed(-30_000), "\u{2212}R$ 300,00");
        assert_eq!(m.signed_plus(10_000), "+R$ 100,00");
        assert_eq!(m.whole(11_776), "R$ 118");
        assert_eq!(m.signed_whole(-38_659), "\u{2212}R$ 387");
        assert_eq!(m.signed_whole(38_659), "R$ 387");
        assert_eq!(m.compact(20_000), "R$ 200");
        assert_eq!(m.compact(33_333), "R$ 333,33");
        assert_eq!(m.axis(250_000), "R$ 2,5 mil");
        let big = m.big(38_659);
        assert_eq!(
            (
                big.sign,
                big.currency,
                big.whole.as_str(),
                big.cents.as_str()
            ),
            ("", "R$", "386", ",59")
        );
        assert_eq!(big.text(), "R$ 386,59");
        assert_eq!(m.big(-5).text(), "\u{2212}R$ 0,05");
        let free = m.big_whole(262_400);
        assert_eq!((free.whole.as_str(), free.cents.as_str()), ("2.624", ""));
        assert_eq!(m.big_whole(-150).text(), "\u{2212}R$ 2");
    }

    #[test]
    fn formatador_oculto_troca_todo_valor_pela_mascara() {
        let m = Money::new(true);
        for text in [
            m.brl(38_659),
            m.signed(-30_000),
            m.signed_plus(10_000),
            m.whole(11_776),
            m.signed_whole(-38_659),
            m.compact(20_000),
            m.compact(33_333),
        ] {
            assert_eq!(text, "R$ •••••");
        }
        assert_eq!(m.axis(500_000), "•••");
        let big = m.big(-38_659);
        assert_eq!(
            (big.sign, big.whole.as_str(), big.cents.as_str()),
            ("", "•••••", "")
        );
        assert_eq!(m.big_whole(262_400), big);
    }

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
    fn reais_inteiros() {
        assert_eq!(whole_brl(50_000), "R$ 500");
        assert_eq!(whole_brl(265_000), "R$ 2.650");
        assert_eq!(whole_brl(26_500), "R$ 265");
        assert_eq!(whole_brl(14_050), "R$ 141");
        assert_eq!(whole_brl(14_049), "R$ 140");
        assert_eq!(whole_brl(-26_500), "R$ 265");
        assert_eq!(whole_brl(0), "R$ 0");
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
