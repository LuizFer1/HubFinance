//! A regra de merge do hub: last-writer-wins por linha, decidido so pelo `updatedAt`.

use std::cmp::Ordering;

use super::hlc::compare_hlc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IgnoreReason {
    Older,
    Same,
}

impl IgnoreReason {
    /// Valor de `ignored[].reason` no contrato.
    pub fn as_str(self) -> &'static str {
        match self {
            IgnoreReason::Older => "older",
            IgnoreReason::Same => "same",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Accept,
    Ignore(IgnoreReason),
}

/// LWW por linha. Igual e ignorado de proposito: e o que torna repetir um push idempotente.
/// `deletedAt` nao participa — uma linha apagada revive se chegar versao mais nova com
/// `deletedAt: null` (reajuste de recorrencia depende disso).
pub fn decide(current_updated_at: Option<&str>, incoming_updated_at: &str) -> Verdict {
    let Some(current) = current_updated_at else {
        return Verdict::Accept;
    };
    match compare_hlc(current, incoming_updated_at) {
        Ordering::Less => Verdict::Accept,
        Ordering::Equal => Verdict::Ignore(IgnoreReason::Same),
        Ordering::Greater => Verdict::Ignore(IgnoreReason::Older),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: &str = "01HZZZZZZZZZZZZZZZZZZZZZZA";

    fn hlc(millis: &str) -> String {
        format!("{millis}-0000-{D}")
    }

    #[test]
    fn linha_nova_e_aceita() {
        assert_eq!(decide(None, &hlc("1759344000000")), Verdict::Accept);
    }

    #[test]
    fn mais_nova_e_aceita() {
        let atual = hlc("0000000000999");
        let recebido = hlc("0000000001000");
        assert_eq!(decide(Some(&atual), &recebido), Verdict::Accept);
    }

    #[test]
    fn igual_e_ignorada_como_same() {
        let atual = hlc("1759344000000");
        assert_eq!(
            decide(Some(&atual), &atual),
            Verdict::Ignore(IgnoreReason::Same)
        );
    }

    #[test]
    fn mais_antiga_e_ignorada_como_older() {
        let atual = hlc("1759344000001");
        let recebido = hlc("1759344000000");
        assert_eq!(
            decide(Some(&atual), &recebido),
            Verdict::Ignore(IgnoreReason::Older)
        );
    }

    #[test]
    fn deleted_at_nao_participa_da_decisao() {
        // Documental: a assinatura so recebe `updatedAt`. Se alguem acrescentar `deletedAt`,
        // este teste deixa de compilar e obriga a reler a spec.
        let f: fn(Option<&str>, &str) -> Verdict = decide;
        assert_eq!(f(None, &hlc("1759344000000")), Verdict::Accept);
    }

    #[test]
    fn razoes_no_formato_do_contrato() {
        assert_eq!(IgnoreReason::Older.as_str(), "older");
        assert_eq!(IgnoreReason::Same.as_str(), "same");
    }
}
