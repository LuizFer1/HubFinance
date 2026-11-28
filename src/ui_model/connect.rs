//! Fluxo "Conectar um celular" e os enderecos do hub, sem toolkit: o estado dos tres passos, os
//! textos de cada sistema e as URLs que a pessoa digita ou abre no celular. Moram aqui, e nao na
//! tela, para a interface nova e os testes usarem as mesmas decisoes.

use crate::hub::snapshot::{Snapshot, Status};
use crate::ui_model::app::{App, Screen};

/// Nucleo no ar (com ou sem rede local): o unico estado em que da para gerar codigo.
pub fn is_up(snap: &Snapshot) -> bool {
    matches!(snap.status, Status::Running | Status::NoNetwork)
}

/// `https://<ip>:<porta>`: o que se digita no app. Sem rede, nada.
pub fn hub_url(snap: &Snapshot) -> Option<String> {
    snap.addresses
        .first()
        .map(|ip| format!("https://{ip}:{}", snap.https_port))
}

/// A pagina do guia na porta HTTP, com o botao "Baixar certificado": e o que o passo 2 manda
/// abrir no celular (o link direto do `.crt` pularia as instrucoes da propria pagina).
pub fn guide_url(snap: &Snapshot) -> Option<String> {
    snap.addresses
        .first()
        .map(|ip| format!("http://{ip}:{}", snap.http_port))
}

// ---- o fluxo na tela ----

/// Sem aparelho ativo e com o nucleo no ar: o fluxo ocupa a tela, sem modal. No `Starting` a
/// lista ainda vem vazia do snapshot inicial, e o fluxo piscaria no boot de quem ja tem celular.
///
/// Conta pelo snapshot, e nao pela lista visivel: o aparelho com remocao pendente ainda conta.
/// Senao o fluxo tomaria a tela durante o "Desfazer", e desfazer o esconderia de novo com o
/// codigo do passo 3 valendo. O fluxo so substitui a lista depois que a remocao sai.
pub fn in_screen_case(app: &App) -> bool {
    is_up(&app.snapshot) && app.snapshot.devices.iter().all(|d| d.revoked)
}

/// O fluxo esta na tela, no lugar da lista?
pub fn shows_in_screen_flow(app: &App) -> bool {
    app.screen == Screen::Connection
        && in_screen_case(app)
        && !app.connect.as_ref().is_some_and(|f| f.modal)
}

/// O dialogo "Conectar um celular" esta aberto? E o que o Esc fecha e o que bloqueia os
/// atalhos por baixo do scrim. Com o hub em falha (`shell::failure`, o mesmo criterio da casca)
/// nao: o card de falha toma a tela, e um dialogo de pareamento por cima dele pediria um codigo
/// que ninguem emite.
pub fn dialog_open(app: &App) -> bool {
    app.screen == Screen::Connection
        && app.connect.as_ref().is_some_and(|f| f.modal)
        && super::shell::failure(app).is_none()
}

/// Os tres passos do "Conectar um celular", na ordem em que a pessoa os faz.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConnectStep {
    #[default]
    WiFi,
    Certificate,
    Pair,
}

impl ConnectStep {
    pub const TOTAL: usize = 3;

    /// Posicao a partir de 0 (o `progress_steps` e o "passo N de 3").
    pub fn index(self) -> usize {
        match self {
            ConnectStep::WiFi => 0,
            ConnectStep::Certificate => 1,
            ConnectStep::Pair => 2,
        }
    }

    pub fn next(self) -> Option<ConnectStep> {
        match self {
            ConnectStep::WiFi => Some(ConnectStep::Certificate),
            ConnectStep::Certificate => Some(ConnectStep::Pair),
            ConnectStep::Pair => None,
        }
    }

    pub fn back(self) -> Option<ConnectStep> {
        match self {
            ConnectStep::WiFi => None,
            ConnectStep::Certificate => Some(ConnectStep::WiFi),
            ConnectStep::Pair => Some(ConnectStep::Certificate),
        }
    }

    fn label(self) -> &'static str {
        match self {
            ConnectStep::WiFi => "Wi-Fi",
            ConnectStep::Certificate => "Certificado",
            ConnectStep::Pair => "Parear",
        }
    }
}

/// O sistema do celular, para os passos do certificado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PhoneOs {
    #[default]
    Android,
    Iphone,
}

/// Estado do fluxo. `modal` e explicito, e nao deduzido da contagem de aparelhos: senao o fluxo
/// em tela no passo 3 viraria dialogo no instante em que um aparelho volta pelo "Desfazer".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnectFlow {
    pub step: ConnectStep,
    pub os: PhoneOs,
    pub modal: bool,
}

impl ConnectFlow {
    pub fn new(modal: bool) -> ConnectFlow {
        ConnectFlow {
            modal,
            ..ConnectFlow::default()
        }
    }
}

/// Passos do certificado de cada sistema, do guia que o hub ja servia (`ca_http::guide_html`).
/// O "abra o app e leia o QR" que fechava cada guia saiu: agora e o passo 3 do fluxo.
pub fn certificate_steps(os: PhoneOs) -> &'static [&'static str] {
    match os {
        PhoneOs::Android => &[
            "No celular, abra o endereço abaixo e toque em Baixar certificado.",
            "Configurações → Segurança → Criptografia e credenciais → Instalar um certificado → \
             Certificado de CA → Instalar mesmo assim → escolha HubFinance-CA.crt.",
        ],
        PhoneOs::Iphone => &[
            "No Safari, abra o endereço abaixo e toque em Baixar certificado → Permitir.",
            "Ajustes → Geral → VPN e Gerenciamento de Dispositivo → HubFinance CA → Instalar.",
            "Ajustes → Geral → Sobre → Ajustes de Confiança de Certificado → ative HubFinance CA.",
        ],
    }
}

/// Passos do pareamento, na ordem certa: primeiro abrir a tela do app, depois ler o QR (antes o
/// "Ajustes → Sincronizar" vinha depois do passo que ja mandava digitar o codigo).
pub const PAIR_STEPS: [&str; 2] = [
    "No app, toque em Ajustes → Sincronizar com o hub.",
    "Aponte a câmera para o QR code ou digite o endereço e o código no app.",
];

/// "Certificado, passo 2 de 3".
pub fn step_caption(step: ConnectStep) -> String {
    format!(
        "{}, passo {} de {}",
        step.label(),
        step.index() + 1,
        ConnectStep::TOTAL
    )
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    #[test]
    fn enderecos_so_com_rede() {
        let mut snap = Snapshot {
            https_port: 7777,
            http_port: 7778,
            ..Snapshot::default()
        };
        assert_eq!(hub_url(&snap), None);
        assert_eq!(guide_url(&snap), None);
        snap.addresses = vec![Ipv4Addr::new(192, 168, 0, 12)];
        assert_eq!(hub_url(&snap).as_deref(), Some("https://192.168.0.12:7777"));
        assert_eq!(
            guide_url(&snap).as_deref(),
            Some("http://192.168.0.12:7778")
        );
    }

    #[test]
    fn passos_andam_de_um_em_um_e_param_nas_pontas() {
        let flow = ConnectFlow::new(true);
        assert_eq!(flow.step, ConnectStep::WiFi);
        assert_eq!(flow.os, PhoneOs::Android);
        assert_eq!(ConnectStep::WiFi.back(), None);
        assert_eq!(ConnectStep::WiFi.next(), Some(ConnectStep::Certificate));
        assert_eq!(ConnectStep::Certificate.next(), Some(ConnectStep::Pair));
        assert_eq!(ConnectStep::Pair.next(), None);
        assert_eq!(ConnectStep::Pair.back(), Some(ConnectStep::Certificate));
        assert_eq!(step_caption(ConnectStep::WiFi), "Wi-Fi, passo 1 de 3");
        assert_eq!(
            step_caption(ConnectStep::Certificate),
            "Certificado, passo 2 de 3"
        );
        assert_eq!(step_caption(ConnectStep::Pair), "Parear, passo 3 de 3");
    }

    #[test]
    fn passos_do_certificado_por_sistema_sem_o_passo_do_qr() {
        let android = certificate_steps(PhoneOs::Android);
        let iphone = certificate_steps(PhoneOs::Iphone);
        assert_eq!(android.len(), 2);
        assert_eq!(iphone.len(), 3);
        assert!(android[1].contains("HubFinance-CA.crt"));
        assert!(iphone[0].starts_with("No Safari"));
        assert!(iphone[2].contains("Confiança"));
        // Ler o QR e o passo 3 do fluxo; os guias nao o repetem mais.
        for line in android.iter().chain(iphone) {
            assert!(!line.contains("QR"), "{line}");
        }
        // Abrir a tela de sincronizar vem antes de ler o QR ou digitar o codigo.
        assert!(PAIR_STEPS[0].contains("Sincronizar com o hub"));
        assert!(PAIR_STEPS[1].contains("QR"));
    }
}
