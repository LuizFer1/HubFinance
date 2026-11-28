//! Tecla pressionada, no vocabulario do `ui_model` (sem tipos de toolkit).
//!
//! Cada toolkit converte a sua tecla para `KeyPress`; a decisao do atalho (`shortcut`) so
//! olha para este tipo. A resolucao "posicao fisica x caractere" mora na conversao, porque
//! e ela que sabe o que o toolkit informa: aqui `Digit` ja e o digito da posicao fisica.

/// A tecla, ja resolvida para o que os atalhos precisam saber.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Escape,
    Tab,
    ArrowLeft,
    ArrowRight,
    /// Digito 1 a 9 pela posicao fisica: em teclado AZERTY o "1" sem Shift e "&", e o
    /// atalho Ctrl+1 continua sendo a tecla do 1. No teclado numerico so conta quando a
    /// tecla produz mesmo o digito (NumLock desligado: Numpad1 e End, e Ctrl+End nao troca
    /// de tela).
    Digit(u8),
    /// O caractere latino que a tecla produz (o DIGITADO: em Dvorak, Ctrl+H e a tecla cujo
    /// "h" logico sai ali, nao a posicao fisica do H). So quando o layout nao e latino
    /// (cirilico etc.) a posicao fisica entra como recurso, para Ctrl+H seguir na tecla do
    /// H. O valor pode nao ser letra ('0', 'a com crase', '-'): quem decide o atalho
    /// compara com os caracteres que usa e ignora o resto.
    Letter(char),
    /// Qualquer outra tecla: nenhum atalho a usa.
    Other,
}

/// Tecla pressionada que sobrou para a janela, com o que o atalho precisa saber.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyPress {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub repeat: bool,
}
