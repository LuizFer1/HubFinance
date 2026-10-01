//! Token de pareamento: curto para digitar, de uso unico, so em memoria.

use std::time::{Duration, SystemTime};

pub const TOKEN_LEN: usize = 6;
pub const TOKEN_TTL: Duration = Duration::from_secs(5 * 60);
pub const MAX_ATTEMPTS: u8 = 5;

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
        });
        token
    }

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
        if TokenBook::normalize(candidate) == active.token.value {
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

    /// Como a pessoa digita: `abc-def`, `ABC DEF` e `ABCDEF` sao o mesmo codigo.
    pub fn normalize(candidate: &str) -> String {
        candidate
            .chars()
            .filter(|c| *c != '-' && !c.is_whitespace())
            .flat_map(char::to_uppercase)
            .collect()
    }
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
    fn display_separa_em_dois_grupos() {
        let token = PairingToken {
            value: "ABCDEF".into(),
            expires_at: t0(),
        };
        assert_eq!(token.display(), "ABC-DEF");
    }
}
