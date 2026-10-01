//! Paleta Nocturne em sRGB 8 bits, um valor por tema.
//!
//! O app e dono do token (o nome gravado na linha: `rose`, `red`...); o handoff e dono do
//! valor visual. Os literais vem de `scripts/oklch.py` (conversao OKLCH -> sRGB feita uma vez)
//! e o teste os fixa: nada e convertido em runtime.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Dark,
    Light,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// Uma cor nos dois temas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pair {
    pub dark: Rgb,
    pub light: Rgb,
}

impl Pair {
    pub const fn get(&self, mode: Mode) -> Rgb {
        match mode {
            Mode::Dark => self.dark,
            Mode::Light => self.light,
        }
    }
}

const fn pair(dark: u32, light: u32) -> Pair {
    Pair {
        dark: hex(dark),
        light: hex(light),
    }
}

const fn hex(v: u32) -> Rgb {
    Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Os 12 tokens fechados do app (`domain/model/tokens.ts`), na ordem do seletor.
pub const TOKENS: [&str; 12] = [
    "slate", "rose", "red", "orange", "amber", "lime", "emerald", "teal", "sky", "indigo",
    "violet", "fuchsia",
];
pub const NEUTRAL_TOKEN: &str = "slate";

pub const INCOME: Pair = pair(0x75d5a0, 0x309564);
pub const INCOME_FG: Pair = pair(0x7bdca6, 0x14764a);
pub const EXPENSE: Pair = pair(0xf28885, 0xca5556);
pub const EXPENSE_FG: Pair = pair(0xfb9795, 0xaf3c40);

fn token_pair(token: &str) -> Option<Pair> {
    Some(match token {
        "slate" => pair(0xb2b6ca, 0x75798c),
        "rose" => pair(0xed93a1, 0xc4576d),
        "red" => pair(0xee9685, 0xc65b48),
        "orange" => pair(0xe9a26b, 0xc36d19),
        "amber" => pair(0xdeb866, 0xb08505),
        "lime" => pair(0xadca7a, 0x749331),
        "emerald" => pair(0x7bcc98, 0x389560),
        "teal" => pair(0x63ccc0, 0x1b9388),
        "sky" => pair(0x6bc0e8, 0x218cb5),
        "indigo" => pair(0x89a9f1, 0x4e71c0),
        "violet" => pair(0xb09bee, 0x7c60bd),
        "fuchsia" => pair(0xe093cf, 0xae539d),
        _ => return None,
    })
}

/// Token gravado na linha -> cor. Desconhecido cai no neutro, como `cssVarForToken` do app;
/// a linha guarda o valor como veio (uma versao mais nova do app pode ter token novo).
pub fn token_color(token: &str, mode: Mode) -> Rgb {
    token_pair(token)
        .or_else(|| token_pair(NEUTRAL_TOKEN))
        .map_or(Rgb(0x80, 0x80, 0x80), |p| p.get(mode))
}

/// Nome que a pessoa le (`COLOR_NAMES` do app): o token tecnico nao muda, o nome exibido
/// segue o valor novo (`red` virou um coral, `indigo` um azul).
pub fn color_name(token: &str) -> &'static str {
    match token {
        "rose" => "Rosa",
        "red" => "Coral",
        "orange" => "Laranja",
        "amber" => "Âmbar",
        "lime" => "Lima",
        "emerald" => "Verde",
        "teal" => "Turquesa",
        "sky" => "Céu",
        "indigo" => "Azul",
        "violet" => "Violeta",
        "fuchsia" => "Magenta",
        _ => "Cinza",
    }
}

/// Mistura por canal em sRGB; o iced nao tem `color-mix`. Pre-misturar sobre a superficie e o
/// que o README do handoff pede para o tile de categoria (18 %) e a tag da pessoa (16 %).
pub fn mix(base: Rgb, over: Rgb, amount: f32) -> Rgb {
    let t = amount.clamp(0.0, 1.0);
    let channel = |a: u8, b: u8| -> u8 {
        let v = f32::from(a) + (f32::from(b) - f32::from(a)) * t;
        // `round` arredonda o meio para longe do zero: 127,5 -> 128.
        v.round().clamp(0.0, 255.0) as u8
    };
    Rgb(
        channel(base.0, over.0),
        channel(base.1, over.1),
        channel(base.2, over.2),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabela_inteira_nos_dois_modos() {
        let table: [(&str, u32, u32); 12] = [
            ("slate", 0xb2b6ca, 0x75798c),
            ("rose", 0xed93a1, 0xc4576d),
            ("red", 0xee9685, 0xc65b48),
            ("orange", 0xe9a26b, 0xc36d19),
            ("amber", 0xdeb866, 0xb08505),
            ("lime", 0xadca7a, 0x749331),
            ("emerald", 0x7bcc98, 0x389560),
            ("teal", 0x63ccc0, 0x1b9388),
            ("sky", 0x6bc0e8, 0x218cb5),
            ("indigo", 0x89a9f1, 0x4e71c0),
            ("violet", 0xb09bee, 0x7c60bd),
            ("fuchsia", 0xe093cf, 0xae539d),
        ];
        for (token, dark, light) in table {
            assert_eq!(token_color(token, Mode::Dark), hex(dark), "{token} escuro");
            assert_eq!(token_color(token, Mode::Light), hex(light), "{token} claro");
        }
        assert_eq!(token_color("rose", Mode::Dark), Rgb(0xed, 0x93, 0xa1));
        assert_eq!(table.map(|(t, _, _)| t), TOKENS);
    }

    #[test]
    fn token_desconhecido_cai_no_neutro() {
        for mode in [Mode::Dark, Mode::Light] {
            assert_eq!(
                token_color("qualquer", mode),
                token_color(NEUTRAL_TOKEN, mode)
            );
        }
    }

    #[test]
    fn nomes_exibidos_seguem_o_app() {
        assert_eq!(color_name("fuchsia"), "Magenta");
        assert_eq!(color_name("red"), "Coral");
        assert_eq!(color_name("slate"), "Cinza");
        assert_eq!(color_name("x"), "Cinza");
    }

    #[test]
    fn receita_e_despesa() {
        assert_eq!(INCOME.dark, Rgb(0x75, 0xd5, 0xa0));
        assert_eq!(INCOME.light, Rgb(0x30, 0x95, 0x64));
        assert_eq!(INCOME_FG.get(Mode::Dark), Rgb(0x7b, 0xdc, 0xa6));
        assert_eq!(EXPENSE.get(Mode::Light), Rgb(0xca, 0x55, 0x56));
        assert_eq!(EXPENSE_FG.light, Rgb(0xaf, 0x3c, 0x40));
    }

    #[test]
    fn mistura_nos_extremos_e_no_meio() {
        let a = Rgb(0x23, 0x25, 0x32);
        let b = Rgb(0xe0, 0x93, 0xcf);
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        assert_eq!(
            mix(Rgb(0, 0, 0), Rgb(255, 255, 255), 0.5),
            Rgb(128, 128, 128)
        );
        // Fora de [0, 1] satura em vez de extrapolar.
        assert_eq!(mix(a, b, 2.0), b);
    }
}
