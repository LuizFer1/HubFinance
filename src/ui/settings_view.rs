//! Tela Ajustes: aparencia, privacidade, inicializacao, assistente e sobre. Junta o que antes
//! ficava espalhado pela lateral (tema), pela Conexao ("Sobre o hub", "Iniciar com o Windows")
//! e pelo olho do Resumo, numa coluna de ate 720 px com linhas de 52.
//!
//! Tudo aqui e preferencia deste computador: nada vai para o banco de sync nem para os
//! celulares (o `ui.json` mora ao lado do banco, mas fora dele).

use iced::widget::text::Wrapping;
use iced::widget::{Column, container, row, text};
use iced::{Alignment, Element, Length, Padding};

use super::app::{App, CopyTarget, Message};
use super::theme::{ButtonKind, ButtonSize, Tokens, space, type_scale};
use super::widgets::{SegmentedSize, SegmentedVariant};
use super::{fonts, icons, widgets};
use crate::assistant::settings::{ProviderChoice, Settings};
use crate::config::{self, ThemeMode};
use crate::hub::snapshot::Snapshot;

/// Largura maxima da coluna: linhas mais largas afastam demais o rotulo do controle.
const COLUMN_MAX: f32 = 720.0;
/// Coluna do nome da secao, a esquerda das linhas.
const SECTION_LABEL: f32 = 120.0;

pub fn view(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let mut sections: Vec<Element<'_, Message>> = vec![
        section(t, "Aparência", vec![theme_row(app)]),
        section(t, "Privacidade", vec![hide_values_row(app)]),
    ];
    if let Some(rows) = startup_rows(app) {
        sections.push(section(t, "Inicialização", rows));
    }
    sections.push(section(t, "Assistente", vec![assistant_row(app)]));
    sections.push(section(t, "Sobre", about_rows(app)));

    let mut col = Column::new();
    for (i, s) in sections.into_iter().enumerate() {
        if i > 0 {
            col = col.push(widgets::divider(t));
        }
        col = col.push(s);
    }
    let card = widgets::card(t, col).padding([space::XS, space::CARD]);
    container(card).max_width(COLUMN_MAX).into()
}

/// Nome da secao a esquerda (13 `text_muted`, na altura da primeira linha) e as linhas a
/// direita, uma sob a outra.
fn section<'a>(
    t: &'static Tokens,
    name: &'a str,
    rows: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    // (52 - 18) / 2: a linha de 13 px centrada na primeira linha de 52.
    let label = container(text(name).size(type_scale::LABEL).color(t.text_muted))
        .width(SECTION_LABEL)
        .padding(Padding {
            top: 17.0,
            ..Padding::ZERO
        });
    row![label, Column::with_children(rows).width(Length::Fill)]
        .spacing(space::L)
        .align_y(Alignment::Start)
        .padding([space::XS, 0.0])
        .into()
}

fn theme_row(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let option = |label, mode| (label, app.theme == mode, Message::SetTheme(mode));
    widgets::settings_row(
        t,
        "Tema",
        None,
        widgets::segmented(
            t,
            SegmentedVariant::PillInset,
            SegmentedSize::Regular,
            false,
            [
                option("Escuro", ThemeMode::Dark),
                option("Claro", ThemeMode::Light),
                option("Sistema", ThemeMode::System),
            ],
        ),
    )
}

/// O mesmo estado do olho do cabecalho: ligar aqui e la e a mesma preferencia.
fn hide_values_row(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    widgets::settings_row(
        t,
        "Ocultar valores",
        None,
        widgets::toggle(t, app.hide_values, Some(|_| Message::ToggleHideValues)),
    )
}

/// So onde o sistema tem a opcao (Windows com bandeja). Sem bandeja fechar a janela desliga o
/// hub, e a frase sobre o relogio seria mentira: a secao inteira some.
fn startup_rows(app: &App) -> Option<Vec<Element<'_, Message>>> {
    let t = app.tokens;
    let enabled = app.autostart?;
    let detail: Option<Element<'_, Message>> = app.autostart_failed.then(|| {
        text("Não foi possível alterar. Veja o log.")
            .size(type_scale::META)
            .color(t.expense_fg)
            .into()
    });
    let switch = widgets::settings_row(
        t,
        "Iniciar com o Windows",
        detail,
        widgets::toggle(t, enabled, Some(Message::SetAutostart)),
    );
    let note = container(
        text(
            "Fechar a janela não desliga o hub; use Desligar o hub no ícone ao lado do relógio. \
             Com a opção ligada, ele abre com o Windows sem mostrar a janela.",
        )
        .size(type_scale::META)
        .line_height(1.45)
        .color(t.text_muted),
    )
    .padding(Padding {
        bottom: space::M,
        ..Padding::ZERO
    });
    Some(vec![switch, note.into()])
}

/// Resumo de uma linha da configuracao do assistente, o mesmo criterio de `Settings`: o que
/// esta escolhido, e se a nuvem esta com o envio desligado.
pub(super) fn assistant_summary(settings: &Settings) -> String {
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

/// "Configurar" leva a Analise com o painel do assistente aberto; o dialogo em passos vem na
/// issue 12.
fn assistant_row(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    let summary = assistant_summary(&app.snapshot.assistant.settings);
    widgets::settings_row(
        t,
        &summary,
        None,
        widgets::action_button(t, ButtonKind::Secondary, ButtonSize::Compact, "Configurar")
            .on_press(Message::ConfigureAssistant),
    )
}

/// Versao com "Versoes no GitHub", as duas pastas com "Copiar" e a frase sobre a internet.
fn about_rows(app: &App) -> Vec<Element<'_, Message>> {
    let t = app.tokens;
    let lines = about_lines(&app.snapshot);
    let path = |p: String| -> Option<Element<'_, Message>> {
        Some(
            text(breakable_path(&p))
                .size(type_scale::META)
                .line_height(1.45)
                .color(t.text_soft())
                .wrapping(Wrapping::WordOrGlyph)
                .into(),
        )
    };
    let footer = container(
        row![
            icons::icon(icons::HOUSE_SIMPLE, 12.0, t.text_muted),
            text(network_note(&app.snapshot.assistant.settings))
                .size(type_scale::META)
                .color(t.text_muted),
        ]
        .spacing(space::S)
        .align_y(Alignment::Center),
    )
    .padding(Padding {
        top: space::XS,
        bottom: space::M,
        ..Padding::ZERO
    });
    vec![
        widgets::settings_row(t, &lines.version, None, releases_link(app)),
        widgets::settings_row(
            t,
            "Dados em",
            path(lines.data_dir),
            copy_button(app, CopyTarget::DataDir),
        ),
        widgets::settings_row(
            t,
            "Logs em",
            path(lines.logs_dir),
            copy_button(app, CopyTarget::LogsDir),
        ),
        footer.into(),
    ]
}

fn copy_button(app: &App, target: CopyTarget) -> Element<'_, Message> {
    widgets::copy_button(
        app.tokens,
        app.is_copied(target),
        Some(Message::Copy(target)),
    )
}

/// "Versoes no GitHub" abre o navegador do sistema; se nao abrir, a URL com "Copiar", sem
/// dialogo (spec de atualizacao, "Tratamento de erros").
fn releases_link(app: &App) -> Element<'_, Message> {
    let t = app.tokens;
    if app.releases_open_failed {
        return row![
            text(config::RELEASES_URL)
                .size(type_scale::META)
                .color(t.text_muted),
            copy_button(app, CopyTarget::ReleasesUrl),
        ]
        .spacing(space::S)
        .align_y(Alignment::Center)
        .into();
    }
    let size = ButtonSize::Compact;
    widgets::action_button_with(
        t,
        ButtonKind::Ghost,
        size,
        row![
            text("Versões no GitHub")
                .size(size.text_size())
                .font(fonts::INTER_MEDIUM),
            icons::icon_inherit(icons::ARROW_SQUARE_OUT, 14.0),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .on_press(Message::OpenReleases)
    .into()
}

/// Textos de "Sobre", separados do layout para o teste.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct AboutLines {
    pub version: String,
    pub data_dir: String,
    pub logs_dir: String,
}

pub(super) fn about_lines(snap: &Snapshot) -> AboutLines {
    AboutLines {
        version: format!("{} v{}", config::HUB_NAME, config::VERSION),
        data_dir: snap.data_dir.display().to_string(),
        logs_dir: config::logs_dir(&snap.data_dir).display().to_string(),
    }
}

/// Caminho com um espaco de largura zero depois de cada separador: o iced so quebra linha em
/// oportunidade de quebra do Unicode, e `\` nao e uma (com `Word` a quebra cairia no meio de
/// um nome, com `Glyph` em qualquer letra). Assim ela cai entre pastas. So para exibir:
/// "Copiar" copia o caminho do snapshot.
fn breakable_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len() + 16);
    for c in path.chars() {
        out.push(c);
        if c == '\\' || c == '/' {
            out.push('\u{200B}');
        }
    }
    out
}

/// Rodape de "Sobre". Com a nuvem do assistente como destino, "nunca consulta a internet"
/// deixaria de ser verdade; o criterio e o mesmo do envio (`Settings::destination`), entao o
/// texto muda exatamente quando algo pode sair.
pub(super) fn network_note(settings: &Settings) -> &'static str {
    if settings.destination().is_some_and(|d| d.is_cloud()) {
        "Só o assistente na nuvem sai da rede, e só quando você envia."
    } else {
        "Este programa nunca consulta a internet"
    }
}

#[cfg(test)]
mod tests {
    use super::super::app::tests::{app_with, with_tray};
    use super::super::app::{Screen, update};
    use super::*;

    #[test]
    fn tela_ajustes_monta_com_e_sem_bandeja() {
        let dir = tempfile::tempdir().unwrap();
        let (mut app, _rx, _tx) = app_with(Snapshot::default(), dir.path().to_path_buf());
        app.screen = Screen::Settings;
        // Sem bandeja nao ha "Iniciar com o Windows".
        assert!(startup_rows(&app).is_none());
        let _ = view(&app);
        let _ = super::super::shell::root(&app);

        let (mut app, _fake, _rx) = with_tray(dir.path());
        app.screen = Screen::Settings;
        assert_eq!(startup_rows(&app).map(|r| r.len()), Some(2));
        app.autostart_failed = true;
        app.releases_open_failed = true;
        let _ = update(&mut app, Message::SetTheme(ThemeMode::Light));
        let _ = view(&app);
        let _ = super::super::shell::root(&app);
    }

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
    fn caminho_quebra_so_depois_do_separador() {
        assert_eq!(
            breakable_path(r"C:\Users\ana"),
            "C:\\\u{200B}Users\\\u{200B}ana"
        );
        assert_eq!(breakable_path("/home/ana"), "/\u{200B}home/\u{200B}ana");
        // Tirando o espaco de largura zero volta o caminho exato.
        let path = r"C:\x\HubFinance\data\logs";
        assert_eq!(breakable_path(path).replace('\u{200B}', ""), path);
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
