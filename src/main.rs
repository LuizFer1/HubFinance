// Build release no Windows e app de janela: sem isso abre um console preto junto. Sem console,
// o log do `tracing` vai para `<data_dir>/logs/` (ver `logging.rs`); em debug o console
// continua recebendo a mesma coisa.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod dashboard;
mod desktop;
mod hub;
mod logging;
mod pairing;
mod protocol;
mod server;
mod store;
mod tls;
mod ui;

fn main() -> iced::Result {
    // Lido antes de criar qualquer thread: em Unix o `time` so le o fuso com um unico thread
    // vivo. Sem ele, a janela e o arquivo de log mostram horas em UTC.
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let launch = desktop::args::Launch::from_args(std::env::args().skip(1));
    let config = match config::Config::from_env() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("HubFinance: {err}");
            std::process::exit(2);
        }
    };
    // Aberto pelo Windows com a opcao desligada: sai antes de log, mutex, banco ou janela.
    // E o que faz "desligar a opcao" valer mesmo com um valor `Run` esquecido.
    let mode = desktop::args::boot_mode(launch, desktop::autostart::is_enabled());
    if mode == desktop::args::BootMode::Exit {
        return Ok(());
    }
    // Depois do config porque o arquivo mora no `data_dir`; antes de tudo o mais para que
    // qualquer aviso do boot ja caia no arquivo.
    logging::init(&config.data_dir, offset);
    // Antes de `hub::start`: um segundo processo nunca abre o SQLite nem tenta a porta.
    let instance = match desktop::instance::acquire(&config.data_dir) {
        Ok(instance) => Some(instance),
        Err(desktop::instance::InstanceError::AlreadyRunning) => {
            tracing::info!("ja ha um hub aberto para esta pasta; pedindo a janela dele");
            // Aberto pelo Windows em cima de um hub que ja roda: so sai, sem puxar janela.
            if !launch.autostart {
                desktop::instance::notify_running(&config.data_dir);
            }
            return Ok(());
        }
        Err(err) => {
            // Sem a garantia de instancia unica o hub ainda funciona; so avisa.
            tracing::warn!("instancia unica indisponivel: {err}");
            None
        }
    };
    // O provider precisa existir antes de qualquer `ServerConfig::builder()`; sem ele o
    // rustls entra em panico na primeira conexao, nao no boot.
    if rustls::crypto::ring::default_provider()
        .install_default()
        .is_err()
    {
        tracing::warn!("provider rustls ja instalado");
    }
    let mut prefs = config::UiPrefs::load(&config.data_dir);
    // "Iniciar com o Windows": ligado por padrao na primeira vez (pedido do dono) e regravado
    // a cada boot para seguir o executavel depois de atualizacao. Nunca em desenvolvimento.
    let dev = cfg!(debug_assertions) || std::env::var_os("HUBFINANCE_DATA_DIR").is_some();
    let available = desktop::autostart::is_enabled().is_some();
    if desktop::autostart::plan(prefs.autostart, available, dev)
        == desktop::autostart::Plan::Register
    {
        match desktop::autostart::set_enabled(true) {
            Ok(()) if prefs.autostart.is_none() => {
                prefs.autostart = Some(true);
                if let Err(err) = prefs.save(&config.data_dir) {
                    tracing::warn!("nao foi possivel gravar ui.json: {err}");
                }
            }
            Ok(()) => {}
            Err(err) => tracing::warn!("iniciar com o Windows: {err}"),
        }
    }
    let (events_tx, events_rx) = desktop::channel();
    if let Some(instance) = &instance {
        instance.watch(events_tx.clone());
    }
    // Bandeja na thread principal, antes do laco do winit (que bombeia as mensagens dela).
    let desktop = desktop::install(events_tx);
    let open_window = desktop::args::opens_window(mode, desktop.has_tray());
    let prefs_dir = config.data_dir.clone();
    let (handle, _nucleo) = hub::start(config);
    // `instance` (mutex e evento) vive ate o fim de `main`, e o kernel so solta os handles
    // quando o processo acaba: a segunda execucao so passa depois disso.
    ui::run(
        handle,
        offset,
        prefs,
        prefs_dir,
        desktop,
        events_rx,
        open_window,
    )
}
