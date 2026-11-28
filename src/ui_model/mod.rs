//! Estado e decisoes da janela, para os dois toolkits e para os testes.
//!
//! Puro como `dashboard/`: sem toolkit de janela nem de desenho. O que mora aqui pode ser testado sem janela,
//! sem fonte e sem relogio de verdade (o "agora" entra por parametro). Os toolkits so
//! desenham o que este modulo decide e traduzem suas teclas e eventos para os tipos daqui.

pub mod analysis;
pub mod assistant_setup;
pub mod connect;
pub mod effect;
pub mod keys;
pub mod measure;
pub mod motion;
pub mod remove;
pub mod settings;
pub mod shell;
pub mod time;
pub mod window;
