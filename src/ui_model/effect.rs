//! O que a janela pede ao mundo de fora. A transicao de estado nao abre janela, nao grava
//! arquivo nem mexe na area de transferencia: descreve o efeito, e cada toolkit o executa. Sem
//! isso o mesmo `update` nao serviria aos dois toolkits, e os testes teriam de montar um
//! executor de tarefas para conferir "isto salvou as preferencias".

use std::path::PathBuf;
use std::time::Duration;

use crate::config::UiPrefs;

/// `M` e a mensagem que `After` devolve ao `update`: generico ate a mensagem sair do toolkit.
#[derive(Debug, Clone)]
pub enum Effect<M> {
    /// Abre a janela (e guarda o id dela no estado, para os botoes da barra nao esperarem).
    OpenWindow,
    /// Desminimiza e pede foco para a janela que ja existe.
    FocusWindow,
    CloseWindow,
    Minimize,
    ToggleMaximize,
    /// Pergunta ao sistema se a janela esta maximizada; a resposta volta como mensagem.
    QueryMaximized,
    StartDrag,
    /// Devolve `M` ao `update` depois do prazo (toasts, "Copiado", desligamento com garantia).
    After(Duration, M),
    SavePrefs(UiPrefs),
    CopyToClipboard(String),
    /// Hoje o `update` abre o navegador na hora, porque a falha vira estado na tela.
    #[allow(dead_code)] // usado na Tarefa 28
    OpenUrl(&'static str),
    #[allow(dead_code)] // usado na Tarefa 28
    OpenFolder(PathBuf),
    FocusNext,
    FocusPrevious,
    ScrollToTop,
    /// Encerra o processo (e acorda o laco de eventos se nao houver janela).
    Exit,
}
