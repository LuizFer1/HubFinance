//! Fluxo do dialogo "Assistente" (onde roda, detalhes, o que grava) e os textos de cada passo, sem
//! toolkit. O rascunho e o que o ultimo botao grava sao decisoes de negocio: moram aqui para a
//! interface nova e a antiga gravarem exatamente a mesma coisa.

use crate::assistant::settings::{ProviderChoice, Settings};

/// Dica da lista vazia; constante para o teste conferir o texto.
pub const NO_MODELS_HINT: &str =
    "Nenhum modelo instalado. Rode ollama pull llama3.1:8b no terminal e clique em Testar conexão.";

/// Antes do primeiro teste nesta porta. "Clique", e nao "Toque": o hub e programa de mesa.
pub const MODELS_NOT_LISTED: &str =
    "Clique em Testar conexão para ver o que o Ollama tem instalado.";

/// Explicacao da nuvem; constante para o teste conferir o texto.
pub const CLOUD_EXPLAIN: &str = "Ligado, o resumo de achados e totais desta conta vai para o OpenRouter quando você conversar. Lançamentos, descrições, pessoas e datas exatas nunca saem.";

pub const PLAIN_KEY_WARNING: &str =
    "Neste sistema a chave fica em texto puro na pasta de dados do hub.";

pub const DENY_LABEL: &str = "Não permitir que o provedor guarde ou treine";

pub const DENY_WARNING: &str = "Alguns provedores podem guardar ou treinar com o que você enviar.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetupStep {
    /// Passo 1: onde roda.
    Where,
    /// Passo 2: porta e modelo do Ollama, ou chave, modelo e retencao da nuvem.
    Details,
}

/// O que a pessoa escolheu no dialogo, ainda nao gravado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupDraft {
    pub choice: ProviderChoice,
    /// So digitos, ate 5. Pode estar invalido (vazio, 0, 70000): o ultimo botao fica desligado.
    pub port_input: String,
    pub local_model: Option<String>,
    pub cloud_model: String,
    pub deny: bool,
}

impl SetupDraft {
    /// O rascunho que reflete o gravado. Nuvem escolhida com o interruptor desligado conta
    /// como "Desligado": e o que vale de fato (nada sai).
    pub fn from_settings(s: &Settings) -> SetupDraft {
        let choice = match s.provider {
            ProviderChoice::Cloud if !s.cloud.enabled => ProviderChoice::None,
            other => other,
        };
        SetupDraft {
            choice,
            port_input: s.local.port.to_string(),
            local_model: s.local.model.clone(),
            cloud_model: s.cloud.model.clone(),
            deny: s.cloud.deny_data_collection,
        }
    }

    /// A porta digitada, se for valida.
    pub fn port(&self) -> Option<u16> {
        self.port_input.parse::<u16>().ok().filter(|p| *p > 0)
    }
}

/// O dialogo aberto.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssistantSetup {
    pub step: SetupStep,
    pub draft: SetupDraft,
    /// O gravado de onde o rascunho saiu. Um snapshot novo (o do boot do nucleo, com o dialogo
    /// aberto antes dele) atualiza o campo que a pessoa nao tocou (ainda igual a base); o que
    /// ela mexeu fica. Sem isto o dialogo aberto cedo regravaria o padrao por cima do salvo.
    pub base: SetupDraft,
    /// `Some(n)`: o `Configure` numero `n` saiu e o dialogo espera `configure_result` com ele.
    pub saving: Option<u64>,
    /// Por que a ultima gravacao deste dialogo nao valeu (do `configure_result`, ou o hub fora
    /// do ar). Mostrado no dialogo; some ao tentar de novo.
    pub error: Option<String>,
}

impl AssistantSetup {
    pub fn open(saved: &Settings) -> AssistantSetup {
        let draft = SetupDraft::from_settings(saved);
        AssistantSetup {
            step: SetupStep::Where,
            base: draft.clone(),
            draft,
            saving: None,
            error: None,
        }
    }

    /// Campo a campo: o que ainda e igual a base acompanha o gravado novo.
    pub fn resync(&mut self, saved: &Settings) {
        fn follow<T: PartialEq + Clone>(draft: &mut T, base: &T, fresh: &T) {
            if draft == base {
                *draft = fresh.clone();
            }
        }
        let fresh = SetupDraft::from_settings(saved);
        let (d, b) = (&mut self.draft, &self.base);
        follow(&mut d.choice, &b.choice, &fresh.choice);
        follow(&mut d.port_input, &b.port_input, &fresh.port_input);
        follow(&mut d.local_model, &b.local_model, &fresh.local_model);
        follow(&mut d.cloud_model, &b.cloud_model, &fresh.cloud_model);
        follow(&mut d.deny, &b.deny, &fresh.deny);
        self.base = fresh;
    }
}

/// O que o ultimo botao grava: o gravado de agora com o que o dialogo decide. A nuvem so fica
/// ligada com "Usar OpenRouter"; sair dela (Desligado ou Ollama) desliga o interruptor, para a
/// nuvem nunca ficar armada sem a pessoa ter escolhido de novo. Os campos de um provedor que a
/// pessoa nao escolheu ficam como estavam. `None` = falta algo (porta invalida, sem modelo).
pub fn settings_to_save(saved: &Settings, draft: &SetupDraft) -> Option<Settings> {
    let mut next = saved.clone();
    next.provider = draft.choice;
    next.cloud.enabled = draft.choice == ProviderChoice::Cloud;
    match draft.choice {
        ProviderChoice::None => {}
        ProviderChoice::Local => {
            next.local.port = draft.port()?;
            let model = draft.local_model.clone().filter(|m| !m.is_empty())?;
            next.local.model = Some(model);
        }
        ProviderChoice::Cloud => {
            let model = draft.cloud_model.trim();
            if model.is_empty() {
                return None;
            }
            next.cloud.model = model.to_string();
            next.cloud.deny_data_collection = draft.deny;
        }
    }
    Some(next)
}

/// O ultimo botao nomeia a acao.
pub fn finish_label(choice: ProviderChoice) -> &'static str {
    match choice {
        ProviderChoice::None => "Desligar assistente",
        ProviderChoice::Local => "Usar Ollama local",
        ProviderChoice::Cloud => "Usar OpenRouter",
    }
}

/// O passo 1 termina ali? So com "Desligado", que nao tem passo 2.
pub fn finishes_here(setup: &AssistantSetup) -> bool {
    setup.step == SetupStep::Details || setup.draft.choice == ProviderChoice::None
}

/// O botao primario do passo: "Continuar" no passo 1 (menos com "Desligado") e a acao no fim.
pub fn primary_label(setup: &AssistantSetup) -> &'static str {
    if setup.saving.is_some() {
        "Salvando…"
    } else if finishes_here(setup) {
        finish_label(setup.draft.choice)
    } else {
        "Continuar"
    }
}

/// Rotulo da linha de progresso.
pub fn step_caption(setup: &AssistantSetup) -> &'static str {
    match (setup.step, setup.draft.choice) {
        (SetupStep::Where, _) => "Onde roda, passo 1 de 2",
        (SetupStep::Details, ProviderChoice::Local) => "No meu computador, passo 2 de 2",
        (SetupStep::Details, _) => "Na nuvem, passo 2 de 2",
    }
}

/// As tres opcoes do passo 1, com a descricao de cada uma.
pub const CHOICES: [(ProviderChoice, &str, &str); 3] = [
    (
        ProviderChoice::None,
        "Desligado",
        "Só os achados, sem conversa.",
    ),
    (
        ProviderChoice::Local,
        "No meu computador",
        "Ollama. Nada sai da máquina.",
    ),
    (
        ProviderChoice::Cloud,
        "Na nuvem",
        "OpenRouter. O resumo sai da sua rede.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn local(port: u16, model: Option<&str>) -> Settings {
        let mut s = Settings {
            provider: ProviderChoice::Local,
            ..Settings::default()
        };
        s.local.port = port;
        s.local.model = model.map(Into::into);
        s
    }

    #[test]
    fn rascunho_reflete_o_gravado_e_nuvem_desligada_conta_como_desligado() {
        let d = SetupDraft::from_settings(&local(11500, Some("m")));
        assert_eq!(d.choice, ProviderChoice::Local);
        assert_eq!(d.port_input, "11500");
        assert_eq!(d.port(), Some(11500));
        let mut cloud_off = Settings {
            provider: ProviderChoice::Cloud,
            ..Settings::default()
        };
        assert_eq!(
            SetupDraft::from_settings(&cloud_off).choice,
            ProviderChoice::None
        );
        cloud_off.cloud.enabled = true;
        assert_eq!(
            SetupDraft::from_settings(&cloud_off).choice,
            ProviderChoice::Cloud
        );
        for bad in ["", "0", "70000"] {
            let mut d = d.clone();
            d.port_input = bad.into();
            assert_eq!(d.port(), None, "{bad}");
        }
    }

    #[test]
    fn o_que_grava_depende_da_escolha_e_so_a_nuvem_liga_o_interruptor() {
        let saved = local(11434, Some("m"));
        let mut d = SetupDraft::from_settings(&saved);
        // Ollama com porta nova.
        d.port_input = "11500".into();
        let next = settings_to_save(&saved, &d).unwrap();
        assert_eq!(next.local.port, 11500);
        assert!(!next.cloud.enabled);
        // Sem modelo ou com porta invalida nao grava.
        d.local_model = None;
        assert_eq!(settings_to_save(&saved, &d), None);
        d.local_model = Some("m".into());
        d.port_input = "0".into();
        assert_eq!(settings_to_save(&saved, &d), None);
        // Nuvem: liga o interruptor, grava modelo e retencao e deixa a porta como estava.
        d.choice = ProviderChoice::Cloud;
        d.cloud_model = "  openai/gpt-4o ".into();
        d.deny = false;
        let next = settings_to_save(&saved, &d).unwrap();
        assert_eq!(next.provider, ProviderChoice::Cloud);
        assert!(next.cloud.enabled);
        assert_eq!(next.cloud.model, "openai/gpt-4o");
        assert!(!next.cloud.deny_data_collection);
        assert_eq!(next.local.port, 11434);
        d.cloud_model = "   ".into();
        assert_eq!(settings_to_save(&saved, &d), None);
        // Desligado desliga a nuvem tambem.
        let mut on = saved.clone();
        on.provider = ProviderChoice::Cloud;
        on.cloud.enabled = true;
        d.choice = ProviderChoice::None;
        let next = settings_to_save(&on, &d).unwrap();
        assert_eq!(next.provider, ProviderChoice::None);
        assert!(!next.cloud.enabled);
    }

    #[test]
    fn ultimo_botao_nomeia_a_acao() {
        assert_eq!(finish_label(ProviderChoice::None), "Desligar assistente");
        assert_eq!(finish_label(ProviderChoice::Local), "Usar Ollama local");
        assert_eq!(finish_label(ProviderChoice::Cloud), "Usar OpenRouter");
        let mut setup = AssistantSetup::open(&Settings::default());
        // Desligado termina no passo 1; os outros continuam.
        assert_eq!(primary_label(&setup), "Desligar assistente");
        setup.draft.choice = ProviderChoice::Local;
        assert_eq!(primary_label(&setup), "Continuar");
        setup.step = SetupStep::Details;
        assert_eq!(primary_label(&setup), "Usar Ollama local");
        setup.draft.choice = ProviderChoice::Cloud;
        assert_eq!(primary_label(&setup), "Usar OpenRouter");
        setup.saving = Some(1);
        assert_eq!(primary_label(&setup), "Salvando…");
    }

    #[test]
    fn snapshot_novo_atualiza_so_o_que_a_pessoa_nao_mexeu() {
        let mut setup = AssistantSetup::open(&Settings::default());
        setup.draft.port_input = "11500".into();
        let mut boot = local(12345, Some("llama"));
        boot.cloud.model = "x/modelo".into();
        setup.resync(&boot);
        assert_eq!(setup.draft.port_input, "11500", "digitado fica");
        assert_eq!(setup.draft.cloud_model, "x/modelo");
        assert_eq!(setup.draft.choice, ProviderChoice::Local);
        assert_eq!(setup.draft.local_model.as_deref(), Some("llama"));
        assert_eq!(setup.base, SetupDraft::from_settings(&boot));
    }

    #[test]
    fn textos_das_opcoes_e_da_nuvem() {
        assert_eq!(
            CHOICES.map(|c| c.1),
            ["Desligado", "No meu computador", "Na nuvem"]
        );
        for (_, label, detail) in CHOICES {
            assert!(!label.contains("  ") && !detail.contains("  "));
        }
        assert_eq!(DENY_LABEL, "Não permitir que o provedor guarde ou treine");
    }
}
