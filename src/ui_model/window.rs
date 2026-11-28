//! Tipos de janela sem toolkit: o id, o tamanho e a borda de redimensionar. Cada toolkit
//! traduz de e para os seus (o adaptador do toolkit mantem o mapa de ids).

/// Id da janela como o modelo a ve. Opaco: so se compara. Quem sabe qual e a janela de verdade
/// e o adaptador do toolkit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

/// Tamanho logico da janela. O modelo so le a largura (colunas por faixa) e a altura (alto dos
/// dialogos).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

impl Size {
    pub const fn new(w: f32, h: f32) -> Size {
        Size { w, h }
    }
}

/// Borda ou canto de onde o arrasto de redimensionar parte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResizeEdge {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}
