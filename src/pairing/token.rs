//! Token de pareamento: curto para digitar, de uso unico, so em memoria.

use std::time::{Duration, SystemTime};

pub const TOKEN_LEN: usize = 6;
pub const TOKEN_TTL: Duration = Duration::from_secs(5 * 60);
pub const MAX_ATTEMPTS: u8 = 5;
/// Erros tolerados em `GET /p/{token}` (o guia da CA) antes de o token morrer. Orcamento
/// proprio e maior que o do pareamento: o guia nao pode gastar as cinco chances do app (o
/// celular abre o guia antes de parear), mas sem limite nenhum ele vira um oraculo — 404 ou
/// 200 diz se o chute acertou, e um script testaria o espaco inteiro sem gastar nada.
pub const MAX_GUIDE_MISSES: u8 = 20;

/// Alfabeto Crockford: sem `I`, `L`, `O`, `U`, que se confundem com `1`, `0` e `V` na tela.
const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairingToken {
    pub value: String,
    pub expires_at: SystemTime,
}

impl PairingToken {
    /// `ABC-DEF`: o hifen so ajuda a ler e digitar; `normalize` o remove na volta.
    pub fn display(&self) -> String {
        let split = self.value.len().min(TOKEN_LEN / 2);
        match (self.value.get(..split), self.value.get(split..)) {
            (Some(head), Some(tail)) if !tail.is_empty() => format!("{head}-{tail}"),
            _ => self.value.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum TokenError {
    #[error("nenhum codigo ativo")]
    Missing,
    #[error("codigo expirado")]
    Expired,
    #[error("codigo errado")]
    Mismatch { remaining: u8 },
    #[error("codigo invalidado apos tentativas demais")]
    Exhausted,
}

/// Um token ativo por vez, so em memoria: reiniciar o hub zera o pareamento em curso.
#[derive(Debug, Default)]
pub struct TokenBook {
    active: Option<Active>,
}

#[derive(Debug)]
struct Active {
    token: PairingToken,
    failures: u8,
    guide_misses: u8,
}

impl TokenBook {
    /// Substitui o token anterior: so o codigo que esta na tela vale.
    pub fn issue(&mut self, now: SystemTime) -> PairingToken {
        let mut bytes = [0u8; TOKEN_LEN];
        rand::fill(&mut bytes);
        // 256 e multiplo de 32: `% 32` nao favorece nenhum caractere.
        let value = bytes
            .iter()
            .map(|b| char::from(CROCKFORD[usize::from(b % 32)]))
            .collect();
        let token = PairingToken {
            value,
            expires_at: now + TOKEN_TTL,
        };
        self.active = Some(Active {
            token: token.clone(),
            failures: 0,
            guide_misses: 0,
        });
        token
    }

    /// Token ativo e valido. So os testes consultam: o nucleo guarda o que `issue` devolveu.
    #[cfg(test)]
    pub fn current(&self, now: SystemTime) -> Option<PairingToken> {
        self.active
            .as_ref()
            .filter(|active| now < active.token.expires_at)
            .map(|active| active.token.clone())
    }

    /// Sucesso e o quinto erro descartam o token; com 32^6 combinacoes e cinco chances,
    /// adivinhar e inviavel na pratica.
    pub fn consume(&mut self, candidate: &str, now: SystemTime) -> Result<(), TokenError> {
        let Some(active) = self.active.as_mut() else {
            return Err(TokenError::Missing);
        };
        if now >= active.token.expires_at {
            self.active = None;
            return Err(TokenError::Expired);
        }
        if ct_eq(
            TokenBook::normalize(candidate).as_bytes(),
            active.token.value.as_bytes(),
        ) {
            self.active = None;
            return Ok(());
        }
        active.failures = active.failures.saturating_add(1);
        if active.failures >= MAX_ATTEMPTS {
            self.active = None;
            return Err(TokenError::Exhausted);
        }
        Err(TokenError::Mismatch {
            remaining: MAX_ATTEMPTS - active.failures,
        })
    }

    /// O token ativo e este? Para `GET /p/{token}` (guia da CA): acertar nao consome nada (o
    /// celular abre o guia antes de parear; consumir aqui mataria o token), e errar gasta do
    /// orcamento do guia, nao das cinco tentativas do pareamento. O vigesimo erro invalida o
    /// token (`Exhausted`); sem token ativo, `Missing`/`Expired` sem contar nada.
    pub fn check_guide(&mut self, candidate: &str, now: SystemTime) -> Result<(), TokenError> {
        let Some(active) = self.active.as_mut() else {
            return Err(TokenError::Missing);
        };
        if now >= active.token.expires_at {
            return Err(TokenError::Expired);
        }
        if ct_eq(
            TokenBook::normalize(candidate).as_bytes(),
            active.token.value.as_bytes(),
        ) {
            return Ok(());
        }
        active.guide_misses = active.guide_misses.saturating_add(1);
        if active.guide_misses >= MAX_GUIDE_MISSES {
            self.active = None;
            return Err(TokenError::Exhausted);
        }
        Err(TokenError::Mismatch {
            remaining: MAX_GUIDE_MISSES - active.guide_misses,
        })
    }

    /// Como a pessoa digita: `abc-def`, `ABC DEF` e `ABCDEF` sao o mesmo codigo.
    pub fn normalize(candidate: &str) -> String {
        candidate
            .chars()
            .filter(|c| *c != '-' && !c.is_whitespace())
            .flat_map(char::to_uppercase)
            .collect()
    }
}

/// Comparacao em tempo constante: `==` de String para no primeiro byte diferente, e o tempo de
/// resposta diria quantos caracteres do codigo ja estao certos.
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ids::is_crockford_upper;

    fn t0() -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_759_344_000)
    }

    fn minutes(n: u64) -> Duration {
        Duration::from_secs(n * 60)
    }

    /// Um candidato que certamente difere do token emitido.
    fn wrong(token: &PairingToken) -> String {
        if token.value == "000000" {
            "111111".into()
        } else {
            "000000".into()
        }
    }

    #[test]
    fn issue_gera_6_chars_crockford_com_validade_de_5_min() {
        let mut book = TokenBook::default();
        let token = book.issue(t0());
        assert_eq!(token.value.len(), TOKEN_LEN);
        assert!(
            token.value.bytes().all(is_crockford_upper),
            "{}",
            token.value
        );
        assert_eq!(token.expires_at, t0() + minutes(5));
    }

    #[test]
    fn current_respeita_a_validade() {
        let mut book = TokenBook::default();
        assert_eq!(book.current(t0()), None);
        let token = book.issue(t0());
        assert_eq!(book.current(t0() + minutes(4)), Some(token));
        assert_eq!(book.current(t0() + minutes(6)), None);
    }

    #[test]
    fn consume_normaliza_e_e_uso_unico() {
        let mut book = TokenBook::default();
        let token = book.issue(t0());
        let typed = format!(
            " {}-{} ",
            token.value[..3].to_lowercase(),
            token.value[3..].to_lowercase()
        );
        assert_eq!(book.consume(&typed, t0()), Ok(()));
        assert_eq!(book.consume(&token.value, t0()), Err(TokenError::Missing));
        assert_eq!(book.current(t0()), None);
    }

    #[test]
    fn normalize_remove_hifen_e_espaco_e_sobe_caixa() {
        assert_eq!(TokenBook::normalize("abc-def"), "ABCDEF");
        assert_eq!(TokenBook::normalize(" AbC dEf "), "ABCDEF");
    }

    #[test]
    fn consumir_expirado_falha() {
        let mut book = TokenBook::default();
        let token = book.issue(t0());
        let result = book.consume(&token.value, t0() + minutes(6));
        assert_eq!(result, Err(TokenError::Expired));
    }

    #[test]
    fn cinco_erros_invalidam_o_token() {
        let mut book = TokenBook::default();
        let token = book.issue(t0());
        let bad = wrong(&token);
        for remaining in (1..MAX_ATTEMPTS).rev() {
            assert_eq!(
                book.consume(&bad, t0()),
                Err(TokenError::Mismatch { remaining })
            );
        }
        assert_eq!(book.consume(&bad, t0()), Err(TokenError::Exhausted));
        assert_eq!(book.current(t0()), None);
        // Nem o codigo certo vale mais: quem errou cinco vezes pode estar chutando.
        assert_eq!(book.consume(&token.value, t0()), Err(TokenError::Missing));
    }

    #[test]
    fn issue_substitui_o_anterior() {
        let mut book = TokenBook::default();
        let mut first = book.issue(t0());
        let mut second = book.issue(t0());
        // 1 em 10^9 de colidir; reemite ate diferir para o teste nao ser intermitente.
        while second.value == first.value {
            first = second;
            second = book.issue(t0());
        }
        assert_eq!(book.current(t0()), Some(second.clone()));
        assert!(matches!(
            book.consume(&first.value, t0()),
            Err(TokenError::Mismatch { .. })
        ));
        assert_eq!(book.consume(&second.value, t0()), Ok(()));
    }

    #[test]
    fn guia_nao_consome_nem_gasta_as_tentativas_do_pareamento() {
        let mut book = TokenBook::default();
        let token = book.issue(t0());
        let bad = wrong(&token);
        for _ in 0..10 {
            assert!(book.check_guide(&bad, t0()).is_err());
            assert_eq!(book.check_guide(&token.value.to_lowercase(), t0()), Ok(()));
        }
        // Os erros no guia nao gastaram nenhuma das cinco tentativas do pareamento.
        assert_eq!(
            book.consume(&bad, t0()),
            Err(TokenError::Mismatch {
                remaining: MAX_ATTEMPTS - 1
            })
        );
        assert_eq!(
            book.check_guide(&token.value, t0() + minutes(6)),
            Err(TokenError::Expired)
        );
        assert_eq!(book.consume(&token.value, t0()), Ok(()));
        assert_eq!(
            book.check_guide(&token.value, t0()),
            Err(TokenError::Missing)
        );
    }

    #[test]
    fn vinte_erros_no_guia_invalidam_o_token() {
        let mut book = TokenBook::default();
        let token = book.issue(t0());
        let bad = wrong(&token);
        for remaining in (1..MAX_GUIDE_MISSES).rev() {
            assert_eq!(
                book.check_guide(&bad, t0()),
                Err(TokenError::Mismatch { remaining })
            );
        }
        assert_eq!(book.check_guide(&bad, t0()), Err(TokenError::Exhausted));
        assert_eq!(
            book.check_guide(&token.value, t0()),
            Err(TokenError::Missing)
        );
        assert_eq!(book.consume(&token.value, t0()), Err(TokenError::Missing));
    }

    #[test]
    fn ct_eq_compara_tamanho_e_conteudo() {
        assert!(ct_eq(b"ABCDEF", b"ABCDEF"));
        assert!(!ct_eq(b"ABCDEF", b"ABCDEG"));
        assert!(!ct_eq(b"ABCDE", b"ABCDEF"));
        assert!(ct_eq(b"", b""));
    }

    #[test]
    fn display_separa_em_dois_grupos() {
        let token = PairingToken {
            value: "ABCDEF".into(),
            expires_at: t0(),
        };
        assert_eq!(token.display(), "ABC-DEF");
    }
}
