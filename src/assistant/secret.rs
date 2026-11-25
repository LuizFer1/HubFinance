//! Chave de API em memoria. O `Debug` nao a mostra e nao existe `Display`: um `{:?}` esquecido
//! num log ou numa mensagem de erro vazaria a chave do OpenRouter para o arquivo de log, que e
//! espelhado em disco.

#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

#[allow(dead_code)] // usado pelo servico (Task 4.6)
impl Secret {
    pub fn new(value: String) -> Self {
        Secret(value)
    }

    /// Unico jeito de ler o valor: o nome chama atencao em quem usa (so o cabecalho `Authorization`).
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_nao_mostra_o_valor() {
        let s = Secret::new("sk-or-segredo".into());
        assert_eq!(format!("{s:?}"), "Secret(***)");
        assert_eq!(format!("{s:#?}"), "Secret(***)");
        assert_eq!(s.expose(), "sk-or-segredo");
    }

    #[test]
    fn debug_de_estrutura_que_contem_a_chave_tambem_esconde() {
        #[derive(Debug)]
        struct Holder {
            #[allow(dead_code)]
            key: Option<Secret>,
        }
        let h = Holder {
            key: Some(Secret::new("sk-or-segredo".into())),
        };
        assert!(!format!("{h:?}").contains("segredo"));
    }
}
