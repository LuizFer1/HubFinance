//! Textos e decisoes puras da casca: versao da marca, plural e milhar do pt-BR, o atalho de cada
//! item da navegacao e qual estado uma tela sem dados mostra. Sem toolkit, para as duas
//! interfaces e os testes dizerem a mesma coisa.

use crate::dashboard::money::group_thousands;
use crate::dashboard::periods::month_long;
use crate::hub::snapshot::Status;

/// A versao precisa de um lugar sempre a vista (Ajustes > Sobre so aparece a quem abre a
/// tela), e o subtitulo curto nao quebra em duas linhas. "Somente leitura" ficou fora dela de
/// proposito: e o subtitulo do Resumo, que no mes corrente da lugar a linha "Parcial".
pub fn brand_subtitle() -> String {
    format!("Hub de casa · v{}", crate::config::VERSION)
}

/// Texto do estado "Abrindo", o mesmo nas tres telas de dados.
pub const OPENING_TEXT: &str = "Abrindo os dados da casa";

/// Linha do Resumo no boot: so o que vale com ou sem dados. "Dados do último sync" ainda nao e
/// verdade enquanto o banco nao abriu.
pub const DASHBOARD_LINE: &str = "Somente leitura";

/// O que uma tela de dados mostra enquanto nao tem o que mostrar (spec 8.2 e A5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arrival {
    /// A tela tem dados: desenha o que tem.
    Data,
    /// Nucleo subindo (`Starting`). O primeiro snapshot sai vazio antes de o banco abrir
    /// (`hub/run.rs`); dizer "Nenhum dado" nesse intervalo seria um vazio falso.
    Opening,
    /// Nucleo no ar, hub sem nenhuma linha e nenhum aparelho pareado: "Comece por aqui".
    FirstUse,
    /// Hub sem nenhuma linha e com aparelho pareado que nunca sincronizou: "Aguardando o
    /// primeiro sync de ...".
    Waiting(String),
    /// Sem dados para esta tela, mas o hub tem linhas (so reservas, conta comercial vazia) ou
    /// o nucleo esta desligando: o vazio de sempre, sem prometer um sync que talvez ja houve.
    Empty,
}

/// Escolhe o estado. A ordem importa: dado na tela ganha de tudo (um snapshot com dados nunca
/// mostra esqueleto), depois o boot, e so com o nucleo no ar e o hub vazio de verdade os passos
/// do primeiro uso ou a espera; o hub com erro nem chega aqui (a casca troca a tela pelo card
/// de falha). `any_synced` (algum aparelho ativo ja enviou ou recebeu) desliga a espera: o
/// primeiro sync manda perfil, categorias e formas de pagamento, que nao sao lancamento nem
/// reserva, e "Aguardando o primeiro sync" seria mentira para quem so nao lancou nada ainda.
pub fn arrival(
    status: &Status,
    screen_has_data: bool,
    hub_has_rows: bool,
    devices: &[&str],
    any_synced: bool,
) -> Arrival {
    if screen_has_data {
        return Arrival::Data;
    }
    match status {
        Status::Starting => Arrival::Opening,
        Status::Running | Status::NoNetwork if !hub_has_rows => {
            if devices.is_empty() {
                Arrival::FirstUse
            } else if any_synced {
                Arrival::Empty
            } else {
                Arrival::Waiting(waiting_line(devices))
            }
        }
        _ => Arrival::Empty,
    }
}

/// "Aguardando o primeiro sync de Pixel da Ana"; com mais de um, a contagem ("de 2 celulares"):
/// a lista de nomes numa linha so nao caberia, e qualquer um deles pode ser o primeiro.
pub fn waiting_line(devices: &[&str]) -> String {
    match devices {
        [] => "Aguardando o primeiro sync".to_string(),
        [one] => format!("Aguardando o primeiro sync de {one}"),
        many => format!(
            "Aguardando o primeiro sync de {}",
            plural(many.len(), "celular", "celulares")
        ),
    }
}

/// Texto do tooltip do item: o atalho da posicao `n` (1 a 6). Sai do mesmo indice que
/// `keys::shortcut` usa, entao nao tem como dizer um numero e a tecla fazer outro.
pub fn nav_shortcut(n: usize) -> String {
    format!("Ctrl+{n}")
}

/// A pergunta que Lancamentos responde (principio 1 da direcao Grafite · Mesa), no mes
/// escolhido. A contagem que esta linha trazia ("4.688 lançamentos sincronizados") desceu para
/// o resumo do mes, logo acima da tabela, onde conta o que a tabela mostra.
pub fn transactions_question(month: &str) -> String {
    format!("O que entrou e saiu em {}?", month_long(month))
}

/// A pergunta que Reservas responde. A tela nao tem seletor de mes, entao nunca da lugar ao
/// "Parcial".
pub const RESERVES_QUESTION: &str = "Quanto está guardado, e para quê?";

/// "1 lançamento" / "1.234 lançamentos" (milhar com ponto, como o pt-BR do app).
pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{} {}", thousands(n), if n == 1 { one } else { many })
}

pub fn thousands(n: usize) -> String {
    group_thousands(n as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marca_mostra_a_versao_numa_linha_curta() {
        let subtitle = brand_subtitle();
        assert!(subtitle.starts_with("Hub de casa · v"), "{subtitle}");
        assert!(subtitle.contains(crate::config::VERSION), "{subtitle}");
        assert!(!subtitle.contains("somente leitura"), "{subtitle}");
        // Cabe numa linha de 11 px nos ~174 px uteis da lateral.
        assert!(subtitle.chars().count() <= 24, "{subtitle}");
    }

    #[test]
    fn milhar_com_ponto() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1234), "1.234");
        assert_eq!(thousands(1_234_567), "1.234.567");
        assert_eq!(plural(1, "pessoa", "pessoas"), "1 pessoa");
        assert_eq!(plural(2, "pessoa", "pessoas"), "2 pessoas");
    }

    #[test]
    fn tooltip_da_nav_mostra_o_atalho_da_posicao() {
        assert_eq!(nav_shortcut(1), "Ctrl+1");
        assert_eq!(nav_shortcut(6), "Ctrl+6");
    }

    #[test]
    fn estado_da_tela_sem_dados_por_status_aparelho_e_hub() {
        let ana = ["Pixel da Ana"];
        let at = |status: Status, data: bool, rows: bool, devices: &[&str], synced: bool| {
            arrival(&status, data, rows, devices, synced)
        };
        let waiting = Arrival::Waiting("Aguardando o primeiro sync de Pixel da Ana".into());
        // Com dados na tela, dados, ate no boot.
        assert_eq!(at(Status::Starting, true, true, &ana, true), Arrival::Data);
        assert_eq!(at(Status::Running, true, true, &[], false), Arrival::Data);
        // Boot: abrindo, mesmo sem aparelho no snapshot inicial.
        assert_eq!(
            at(Status::Starting, false, false, &[], false),
            Arrival::Opening
        );
        // Nucleo no ar, hub vazio e nenhum aparelho: primeiro uso (tambem sem rede).
        assert_eq!(
            at(Status::Running, false, false, &[], false),
            Arrival::FirstUse
        );
        assert_eq!(
            at(Status::NoNetwork, false, false, &[], false),
            Arrival::FirstUse
        );
        // Aparelho pareado que nunca sincronizou: aguardando (tambem sem rede).
        assert_eq!(at(Status::Running, false, false, &ana, false), waiting);
        assert_eq!(at(Status::NoNetwork, false, false, &ana, false), waiting);
        // I1: o aparelho ja sincronizou (perfil, categorias), so nao ha lancamento nem reserva:
        // nada de "Aguardando o primeiro sync".
        assert_eq!(
            at(Status::Running, false, false, &ana, true),
            Arrival::Empty
        );
        assert_eq!(
            at(Status::NoNetwork, false, false, &ana, true),
            Arrival::Empty
        );
        // O hub tem linhas (so reservas, outra conta): o vazio de sempre, sem prometer sync.
        assert_eq!(
            at(Status::Running, false, true, &ana, false),
            Arrival::Empty
        );
        assert_eq!(at(Status::Running, false, true, &[], false), Arrival::Empty);
        // Desligando, desligado ou com erro: nem passos, nem espera, nem esqueleto.
        assert_eq!(
            at(Status::Stopped, false, false, &[], false),
            Arrival::Empty
        );
        assert_eq!(
            at(Status::Stopping, false, false, &ana, false),
            Arrival::Empty
        );
        let failed = || Status::Failed("Porta 7777 em uso".into());
        assert_eq!(at(failed(), false, false, &ana, false), Arrival::Empty);
        assert_eq!(at(failed(), false, false, &[], false), Arrival::Empty);
    }

    #[test]
    fn texto_do_aguardando_com_um_e_com_varios_aparelhos() {
        assert_eq!(
            waiting_line(&["Pixel da Ana"]),
            "Aguardando o primeiro sync de Pixel da Ana"
        );
        assert_eq!(
            waiting_line(&["Pixel da Ana", "iPhone do Luiz"]),
            "Aguardando o primeiro sync de 2 celulares"
        );
        assert_eq!(waiting_line(&[]), "Aguardando o primeiro sync");
    }
}
