//! Textos de Ajustes que nao dependem de toolkit: o resumo do assistente, as linhas de "Sobre" e
//! o rodape de rede. Puros para a interface nova e a antiga dizerem o mesmo.

use crate::assistant::settings::{ProviderChoice, Settings};
use crate::config;
use crate::hub::snapshot::Snapshot;

/// Resumo de uma linha da configuracao do assistente, o mesmo criterio de `Settings`: o que
/// esta escolhido, e se a nuvem esta com o envio desligado.
pub fn assistant_summary(settings: &Settings) -> String {
    match settings.provider {
        ProviderChoice::None => "Desligado · só os achados, sem conversa".into(),
        ProviderChoice::Local => match &settings.local.model {
            Some(model) => format!("Ollama local, {model}"),
            None => "Ollama local, nenhum modelo escolhido".into(),
        },
        ProviderChoice::Cloud if settings.cloud.enabled => {
            format!("OpenRouter, {}", settings.cloud.model)
        }
        ProviderChoice::Cloud => format!("OpenRouter, {} · envio desligado", settings.cloud.model),
    }
}

/// Textos de "Sobre", separados do layout para o teste.
#[derive(Debug, PartialEq, Eq)]
pub struct AboutLines {
    pub version: String,
    pub data_dir: String,
    pub logs_dir: String,
}

pub fn about_lines(snap: &Snapshot) -> AboutLines {
    AboutLines {
        version: format!("{} v{}", config::HUB_NAME, config::VERSION),
        data_dir: snap.data_dir.display().to_string(),
        logs_dir: config::logs_dir(&snap.data_dir).display().to_string(),
    }
}

/// Rodape de "Sobre". Com a nuvem do assistente como destino, "nunca consulta a internet"
/// deixaria de ser verdade; o criterio e o mesmo do envio (`Settings::destination`), entao o
/// texto muda exatamente quando algo pode sair.
pub fn network_note(settings: &Settings) -> &'static str {
    if settings.destination().is_some_and(|d| d.is_cloud()) {
        "Só o assistente na nuvem sai da rede, e só quando você envia."
    } else {
        "Este programa nunca consulta a internet"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resumo_do_assistente_por_escolha() {
        let mut s = Settings::default();
        assert_eq!(
            assistant_summary(&s),
            "Desligado · só os achados, sem conversa"
        );
        s.provider = ProviderChoice::Local;
        assert_eq!(
            assistant_summary(&s),
            "Ollama local, nenhum modelo escolhido"
        );
        s.local.model = Some("llama3.1:8b".into());
        assert_eq!(assistant_summary(&s), "Ollama local, llama3.1:8b");
        s.provider = ProviderChoice::Cloud;
        s.cloud.model = "x/y".into();
        assert_eq!(assistant_summary(&s), "OpenRouter, x/y · envio desligado");
        s.cloud.enabled = true;
        assert_eq!(assistant_summary(&s), "OpenRouter, x/y");
    }

    #[test]
    fn rodape_diz_que_so_a_nuvem_sai_da_rede() {
        let mut s = Settings::default();
        assert_eq!(network_note(&s), "Este programa nunca consulta a internet");
        s.provider = ProviderChoice::Local;
        s.cloud.enabled = true;
        assert_eq!(network_note(&s), "Este programa nunca consulta a internet");
        s.provider = ProviderChoice::Cloud;
        s.cloud.enabled = false;
        assert_eq!(network_note(&s), "Este programa nunca consulta a internet");
        s.cloud.enabled = true;
        assert_eq!(
            network_note(&s),
            "Só o assistente na nuvem sai da rede, e só quando você envia."
        );
    }

    #[test]
    fn sobre_mostra_versao_e_as_duas_pastas() {
        let snap = Snapshot {
            data_dir: std::path::PathBuf::from("C:/x/HubFinance/data"),
            ..Snapshot::default()
        };
        let lines = about_lines(&snap);
        assert_eq!(lines.version, format!("HubFinance v{}", config::VERSION));
        assert_eq!(lines.data_dir, "C:/x/HubFinance/data");
        assert_eq!(
            lines.logs_dir,
            config::logs_dir(&snap.data_dir).display().to_string()
        );
        assert!(lines.logs_dir.starts_with(&lines.data_dir));
        assert!(lines.logs_dir.ends_with("logs"));
    }
}
