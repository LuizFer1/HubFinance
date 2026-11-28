//! Textos e decisoes puras da tela Analise, sem toolkit: a linha-resumo dos achados, a comparacao
//! de gasto contra referencia, o rodape das respostas, a previa do envio a nuvem e, acima de
//! tudo, a mascara de "Ocultar valores" (que precisa ser uma so para as duas interfaces: um
//! valor que vaza numa delas anula a promessa da outra).

use crate::advisor::{FigureValue, Finding, FindingKind, Severity};
use crate::assistant::prompt::Role;
use crate::assistant::settings::{ProviderChoice, Settings};
use crate::assistant::view::{MessageView, OpenConversation};
use crate::dashboard::money::{MASK, MASK_DIGITS, MINUS, group_thousands};

/// A pergunta que a tela responde, no cabecalho fixo da casca.
pub const QUESTION: &str = "Onde dá para melhorar?";

/// Texto da sobreposicao; constante para um teste conferir que a quebra de linha no fonte nao
/// deixou espacos soltos no meio da frase.
pub const INTRO: &str = "Este é o texto exato que o assistente recebe: resumos e totais, sem \
lançamentos, descrições, nomes de pessoas, aparelhos ou formas de pagamento, nem datas \
exatas.";

/// "2 alertas, 3 atenções, 1 bom sinal. Calculado no seu PC." Grupo vazio nao entra: "0
/// alertas" chamaria atencao para o que nao existe.
pub fn summary_line(findings: &[Finding]) -> String {
    let count = |s: Severity| findings.iter().filter(|f| f.severity == s).count();
    let parts: Vec<String> = [
        (count(Severity::Alert), "alerta", "alertas"),
        (count(Severity::Attention), "atenção", "atenções"),
        (count(Severity::GoodSign), "bom sinal", "bons sinais"),
    ]
    .into_iter()
    .filter(|(n, ..)| *n > 0)
    .map(|(n, one, many)| format!("{n} {}", if n == 1 { one } else { many }))
    .collect();
    if parts.is_empty() {
        "Nada a apontar. Calculado no seu PC.".to_string()
    } else {
        format!("{}. Calculado no seu PC.", parts.join(", "))
    }
}

/// Titulo do grupo: plural, como na linha-resumo ("Alertas 2").
pub fn group_title(s: Severity) -> &'static str {
    match s {
        Severity::Alert => "Alertas",
        Severity::Attention => "Atenção",
        Severity::GoodSign => "Bons sinais",
    }
}

/// Gasto contra a referencia de um achado (spec 10: "barra de real contra normal"), lido das
/// cifras que ja vao ao envelope, sem mudar `Finding`. So os achados que comparam um gasto com
/// outro valor do mesmo tipo; os de taxa, meses ou previsao nao tem um par assim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparison {
    pub real: i64,
    pub reference: i64,
    pub reference_label: &'static str,
}

/// A ordem das cifras nao e a mesma em toda regra (`advisor/rules.rs`): ritmo e categoria vem
/// `[gasto, normal]`, meta vem `[meta, gasto]`. Por isso o tipo decide, num lugar so.
pub fn comparison(f: &Finding) -> Option<Comparison> {
    let money = |i: usize| match f.figures.get(i)?.value {
        FigureValue::Money(v) => Some(v),
        _ => None,
    };
    let (real, reference, reference_label) = match f.kind {
        FindingKind::Pace | FindingKind::CategoryAboveNormal => (money(0)?, money(1)?, "Normal"),
        FindingKind::Budget => (money(1)?, money(0)?, "Meta"),
        _ => return None,
    };
    // Sem escala (os dois zero ou negativos) a barra nao diz nada.
    (real.max(reference) > 0).then_some(Comparison {
        real,
        reference,
        reference_label,
    })
}

/// "llama3.1:8b, local" sob o titulo do painel: onde o assistente roda agora.
pub fn assistant_status(settings: &Settings) -> String {
    match settings.provider {
        ProviderChoice::None => "Desligado".to_string(),
        ProviderChoice::Local => match settings.local.model.as_deref() {
            Some(m) if !m.is_empty() => format!("{m}, local"),
            _ => "Ollama, sem modelo".to_string(),
        },
        ProviderChoice::Cloud if !settings.cloud.enabled => "Nuvem desligada".to_string(),
        ProviderChoice::Cloud => format!("{}, nuvem", settings.cloud.model.trim()),
    }
}

pub fn tokens_label((tokens_in, tokens_out): (u32, u32)) -> String {
    format!(
        "{} → {} tokens",
        group_thousands(u64::from(tokens_in)),
        group_thousands(u64::from(tokens_out))
    )
}

/// Total do cabecalho. Sem nenhuma resposta que tenha informado uso (modelo local que nao manda
/// `usage`, ou conversa so com a abertura), "0 → 0" afirmaria um zero que ninguem mediu.
pub fn tokens_total_label(open: &OpenConversation) -> String {
    if open
        .messages
        .iter()
        .any(|m| m.tokens_in.is_some() || m.tokens_out.is_some())
    {
        tokens_label(open.tokens_total())
    } else {
        "—".to_string()
    }
}

/// A mascara do texto chegando (`plain_text` do parcial). O numero no fim pode estar pela
/// metade ("1500,0" vira "1500,00"; "5 " vira "5 mil"; "1 " vira "1 234,56"): incompleto, a
/// mascara nao o reconhece como dinheiro. Por isso o fim que ainda pode virar valor fica
/// escondido ate o texto seguir (`pending_number_start`).
pub fn mask_streaming(drawn: &str) -> String {
    let drawn = drawn.trim_end();
    match pending_number_start(drawn) {
        Some(cut) => mask_str(&drawn[..cut]),
        None => mask_str(drawn),
    }
}

/// Inicio do numero que encosta no fim de `text` e ainda pode crescer, se ha um. Andando de tras
/// para frente: um comeco de unidade ("m", "mi", "re"), espacos, e a sequencia de digitos,
/// separadores (".", ",", espaco de milhar) e marcas de Markdown ainda abertas ("**", "~~"),
/// com o sinal colado. Sem digito nessa sequencia nao ha numero pendente.
fn pending_number_start(text: &str) -> Option<usize> {
    let mut end = text.len();
    let word = text[..end]
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_alphabetic())
        .last()
        .map(|(i, _)| i);
    if let Some(start) = word {
        let w = text[start..end].to_lowercase();
        let unit = MAGNITUDES
            .iter()
            .chain(UNITS.iter())
            .any(|u| u.starts_with(&w));
        let before = text[..start].chars().next_back();
        if unit && before.is_some_and(|c| c.is_whitespace() || c.is_ascii_digit()) {
            end = start;
        }
    }
    let end = text[..end].trim_end().len();
    let mut start = end;
    let mut digit = false;
    for (i, c) in text[..end].char_indices().rev() {
        let part = c.is_ascii_digit()
            || matches!(c, '.' | ',' | '*' | '_' | '~' | '`' | '\\')
            || thousands_space(text, i).is_some();
        if !part {
            break;
        }
        digit |= c.is_ascii_digit();
        start = i;
    }
    if !digit {
        return None;
    }
    // Comeca no primeiro digito ou marca, nao no espaco que separa da palavra anterior.
    start = end - text[start..end].trim_start().len();
    let head = strip_sign(&text[..start]);
    Some(head.len())
}

/// O texto que o Markdown desenharia, lido dos eventos do `pulldown-cmark` com as mesmas opcoes
/// da tela: texto e codigo (celulas de tabela inclusive), quebras e fim de bloco viram quebra de
/// linha, HTML some (a tela nao o desenha). E a reserva da tabela, que o desenho nao deixa ler: a
/// mascara roda sobre o texto ja interpretado, onde "1.~~234~~,56" ja e "1.234,56".
pub fn plain_text(content: &str) -> String {
    use pulldown_cmark::{Event, Options, Parser, TagEnd};
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut out = String::new();
    for event in Parser::new_ext(content, options) {
        match event {
            Event::Text(t) | Event::Code(t) => out.push_str(&t),
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::CodeBlock
                | TagEnd::TableRow
                | TagEnd::TableHead,
            ) => out.push('\n'),
            Event::End(TagEnd::TableCell) => out.push_str(" | "),
            _ => {}
        }
    }
    out
}

/// Rodape da resposta: local = "modelo · tokens"; nuvem = "tokens · selo e retencao · modelo",
/// com o modelo por ultimo para o selo de privacidade ficar junto do que ele qualifica.
/// Registro antigo, sem modelo, simplesmente nao o mostra.
pub fn reply_notes(m: &MessageView) -> Vec<String> {
    let mut notes = Vec::new();
    let model = m.model.as_deref().filter(|s| !s.is_empty());
    if let Some(model) = model
        && !m.sent_to_cloud
    {
        notes.push(model.to_string());
    }
    notes.push(match (m.tokens_in, m.tokens_out) {
        (Some(i), Some(o)) => tokens_label((i, o)),
        _ => "—".to_string(),
    });
    if m.interrupted {
        notes.push("interrompida".to_string());
    }
    if m.sent_to_cloud {
        notes.push(cloud_badge(m.deny_data_collection).to_string());
        if let Some(model) = model {
            notes.push(model.to_string());
        }
    }
    notes
}

/// Selo da resposta que passou pela nuvem, com a politica de retencao valendo no envio (spec 7).
/// `None` = registro sem a politica: so diz que foi a nuvem.
pub fn cloud_badge(deny_data_collection: Option<bool>) -> &'static str {
    match deny_data_collection {
        Some(true) => "enviado à nuvem · sem retenção",
        Some(false) => "enviado à nuvem · retenção permitida",
        None => "enviado à nuvem",
    }
}

pub const PREVIEW_TITLE: &str = "Enviar para a nuvem?";

/// O botao da previa diz para onde vai: "Enviar" sozinho nao diz que sai do computador.
pub const PREVIEW_SEND: &str = "Enviar ao OpenRouter";

pub const PREVIEW_INTRO: &str = "Este é o texto exato que vai para o OpenRouter: o resumo desta conta e a pergunta de abertura, além das instruções fixas do assistente. Só depois de você enviar é que algo sai do computador.";

/// Conversa que ja tinha mensagens (por exemplo, respondidas pelo modelo local antes de trocar
/// para a nuvem): elas vao junto, e a previa diz isso.
pub const PREVIEW_INTRO_EARLIER: &str = "Este é o texto exato que vai para o OpenRouter: o resumo desta conta e as mensagens anteriores desta conversa (só as últimas que cabem no pedido), além das instruções fixas do assistente. Só depois de você enviar é que algo sai do computador.";

/// A retencao da previa, a partir da politica configurada agora (e a que vale no envio).
pub fn retention_value(deny_data_collection: bool) -> &'static str {
    if deny_data_collection {
        "não permitida"
    } else {
        "permitida"
    }
}

/// Tudo que o dono dos dados autoriza ao aprovar: o envelope e as mensagens que vao no pedido.
/// Sai de `OpenConversation::payload`, a mesma montagem do envio (janela de turnos incluida),
/// para a previa nao poder divergir do que de fato sai. As regras fixas do assistente (sem
/// dado nenhum) so sao citadas na introducao.
pub fn preview_body(open: &OpenConversation) -> String {
    preview_text_with(open, str::to_string)
}

/// A montagem da previa, com `assistant` aplicado ao texto de cada resposta anterior.
pub fn preview_text_with(open: &OpenConversation, assistant: impl Fn(&str) -> String) -> String {
    let payload = open.payload();
    let mut out = open.envelope.clone();
    out.push_str("\n\n— Conversa —");
    for turn in payload.iter().skip(1) {
        out.push('\n');
        match turn.role {
            Role::Assistant => {
                out.push_str("Assistente: ");
                out.push_str(&assistant(&turn.content));
            }
            _ => {
                out.push_str("Você: ");
                out.push_str(&turn.content);
            }
        }
    }
    out
}

/// A introducao conforme haja mensagens anteriores alem da pergunta pendente.
pub fn preview_intro(open: &OpenConversation) -> &'static str {
    if open.payload().len() > 2 {
        PREVIEW_INTRO_EARLIER
    } else {
        PREVIEW_INTRO
    }
}

/// Rotulo do botao "Apagar todas" conforme esteja armado ou nao.
pub fn delete_all_label(armed: bool) -> &'static str {
    if armed {
        "Apagar todas? Clique de novo"
    } else {
        "Apagar todas"
    }
}

/// Espaco que ainda liga o numero ao que vem antes ou depois: qualquer espaco Unicode, menos a
/// quebra de linha. Sem isso "R$" no fim de uma linha comeria o numero da lista seguinte, e um
/// "3" no fim de uma linha leria "Real" da proxima como unidade.
fn same_line_space(c: char) -> bool {
    c.is_whitespace() && !matches!(c, '\n' | '\r')
}

/// Um numero lido por `scan_number`.
struct Number {
    /// Fim do numero (byte); termina sempre num digito.
    end: usize,
    /// Digitos da parte inteira, milhares incluidos.
    int_digits: usize,
    /// Tem separador de milhar ("1.500", "1 234").
    grouped: bool,
    /// Termina em virgula ou ponto com exatamente duas casas ("12,50", "15.90").
    cents: bool,
}

/// Espaco que separa milhar ("1 234,56"): o comum, o NBSP, o fino e o U+202F que alguns
/// formatadores usam. Devolve o tamanho em bytes.
fn thousands_space(s: &str, at: usize) -> Option<usize> {
    match s.get(at..)?.chars().next()? {
        c @ (' ' | '\u{a0}' | '\u{2009}' | '\u{202f}') => Some(c.len_utf8()),
        _ => None,
    }
}

/// Le o numero que comeca no digito `start`: parte inteira, grupos de milhar (ponto ou espaco
/// e exatamente 3 digitos, so se a primeira parte tem ate 3) e parte decimal (virgula ou ponto
/// seguido de digito). Separador sem digito depois nao entra: e pontuacao da frase.
fn scan_number(s: &str, start: usize) -> Number {
    let b = s.as_bytes();
    let digits = |from: usize| {
        b.get(from..)
            .map_or(0, |r| r.iter().take_while(|c| c.is_ascii_digit()).count())
    };
    let lead = digits(start);
    let mut end = start + lead;
    let mut int_digits = lead;
    let mut grouped = false;
    if lead <= 3 {
        loop {
            let sep = if b.get(end) == Some(&b'.') {
                Some(1)
            } else {
                thousands_space(s, end)
            };
            match sep {
                Some(n) if digits(end + n) == 3 => {
                    end += n + 3;
                    int_digits += 3;
                    grouped = true;
                }
                _ => break,
            }
        }
    }
    let mut cents = false;
    if matches!(b.get(end), Some(b',' | b'.')) {
        let frac = digits(end + 1);
        if frac > 0 {
            cents = frac == 2;
            end += 1 + frac;
        }
    }
    Number {
        end,
        int_digits,
        grouped,
        cents,
    }
}

/// A palavra logo depois de `at` (pulando espacos), em minusculas, e onde ela termina. Palavra
/// inteira: "5 kg" le "kg", e nao "k".
fn next_word(s: &str, at: usize) -> Option<(String, usize)> {
    let rest = s.get(at..)?;
    let start = at + (rest.len() - rest.trim_start_matches(same_line_space).len());
    let len: usize = s[start..]
        .chars()
        .take_while(|c| c.is_alphanumeric())
        .map(char::len_utf8)
        .sum();
    (len > 0).then(|| (s[start..start + len].to_lowercase(), start + len))
}

/// Ordem de grandeza depois do numero: some junto com ele.
const MAGNITUDES: [&str; 4] = ["mil", "milhão", "milhões", "k"];

/// Unidade depois do numero: faz dele dinheiro, mas fica na frase.
const UNITS: [&str; 2] = ["reais", "real"];

/// Fim da ordem de grandeza ("mil", "k") depois do numero que termina em `at`.
fn magnitude_end(s: &str, at: usize) -> Option<usize> {
    next_word(s, at).and_then(|(w, end)| MAGNITUDES.contains(&w.as_str()).then_some(end))
}

/// Fim do trecho a mascarar se o numero solto `n` tem cara de dinheiro; `None` para o resto
/// ("12,5%", "0,5 mês", "3 meses").
fn bare_money_end(s: &str, n: &Number) -> Option<usize> {
    if s[n.end..]
        .trim_start_matches(same_line_space)
        .starts_with('%')
    {
        return None;
    }
    if let Some(end) = magnitude_end(s, n.end) {
        return Some(end);
    }
    let unit = next_word(s, n.end).is_some_and(|(w, _)| UNITS.contains(&w.as_str()));
    (n.int_digits >= 3 || n.grouped || n.cents || unit).then_some(n.end)
}

/// Tira o sinal colado ao fim de `before` ("−" tipografico, "-" ou "+"), so quando ele nao
/// emenda numa palavra: "item-1.500" nao e sinal.
fn strip_sign(before: &str) -> &str {
    for sign in [MINUS, "-", "+"] {
        if let Some(rest) = before.strip_suffix(sign)
            && !rest.chars().next_back().is_some_and(char::is_alphanumeric)
        {
            return rest;
        }
    }
    before
}

/// Troca todo valor em dinheiro do texto pela mascara. Os achados trazem "R$ 1.234,56", mas a
/// resposta do modelo pode vir sem o "R$" ("1.500 reais", "5 mil", "1234.56"), entao ha duas
/// regras:
///
/// - "R$" seguido de numero (depois de qualquer espaco Unicode: NBSP, U+202F, tab) vira `MASK`.
///   O fim do numero sai de `scan_number`: um ponto ou virgula so pertence ao valor quando vem
///   um digito depois, senao "R$ 960,00." comeria o ponto final e "R$ 5 de R$ 9" comeria o
///   " de". "R$" sem digito depois (parte do nome de uma categoria, por ex.) fica literal.
/// - Numero solto com cara de dinheiro vira `MASK_DIGITS`: parte inteira de 3 ou mais digitos
///   ("1500", "1500,5", "1234.56"; um ano tambem cai aqui, e e aceito: vazar valor e pior),
///   milhar com ponto ou espaco ("1.500", "1 234,56"), duas casas ("12,50") ou unidade depois
///   ("3 reais", "5 mil", "1,5 mil", "5k"). Porcentagem ("12,5%", "150%"), meses ("0,5 mês"),
///   datas ("19 nov", "24/09") e numero pequeno ("3 meses") ficam.
///
/// A ordem de grandeza ("mil", "milhões", "k") some junto com o numero: "••••• mil" ainda diria
/// o tamanho do valor. "reais" fica, como o resto da frase.
///
/// O sinal colado ao valor ("−", "-" ou "+") sai junto: oculto nao mostra o sinal, como o
/// `signed_brl` do dashboard. O "~" de estimativa e a pontuacao em volta ficam.
pub fn mask_str(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut copied = 0;
    let mut i = 0;
    while i < b.len() {
        if s[i..].starts_with("R$") {
            let after = i + 2;
            let number = s.len() - s[after..].trim_start_matches(same_line_space).len();
            if b.get(number).is_some_and(u8::is_ascii_digit) {
                let n = scan_number(s, number);
                let end = magnitude_end(s, n.end).unwrap_or(n.end);
                out.push_str(strip_sign(&s[copied..i]));
                out.push_str(MASK);
                copied = end;
                i = end;
            } else {
                i = after;
            }
            continue;
        }
        if b[i].is_ascii_digit() {
            let word = s[..i]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric);
            let n = scan_number(s, i);
            let mut run_end = n.end;
            if !word && let Some(end) = bare_money_end(s, &n) {
                out.push_str(strip_sign(&s[copied..i]));
                out.push_str(MASK_DIGITS);
                copied = end;
                run_end = end;
            }
            // Pula o numero inteiro: recomecar no meio dele acharia "500,00" em "x1500,00".
            i = run_end.max(i + 1);
            continue;
        }
        i += s[i..].chars().next().map_or(1, char::len_utf8);
    }
    out.push_str(&s[copied..]);
    out
}

/// Aviso acima da previa com "Ocultar valores" ligado: a tela mascara, o envio nao.
pub const HIDDEN_VALUES_NOTE: &str =
    "Os valores estão ocultos na tela; o envio leva os valores reais.";

/// Abaixo desta largura util os dois paineis nao cabem (achados precisam de ~320 ao lado dos
/// 400 do assistente e do respiro): a tela vira o segmentado. A janela minima (1024) da 728.
pub const TWO_PANELS_MIN: f32 = 760.0;

/// Os dois paineis cabem lado a lado nesta largura util?
pub fn two_panels(content_width: f32) -> bool {
    content_width >= TWO_PANELS_MIN
}

// ---- Markdown da resposta, como blocos proprios ----

/// Trecho de texto com o estilo que a resposta do modelo pode ter. O que a interface nao desenha
/// (link, riscado, imagem) entra como texto simples: perder o enfeite e melhor que perder o texto.
#[allow(dead_code)] // usado na Tarefa 30
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
}

/// Um bloco da resposta. A lista e o que as interfaces desenham; quem confere a mascara e
/// `blocks_text`, que le exatamente o que sera desenhado.
#[allow(dead_code)] // usado na Tarefa 30
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplyBlock {
    Heading {
        level: u8,
        spans: Vec<Span>,
    },
    Paragraph(Vec<Span>),
    CodeBlock(Vec<String>),
    List {
        first: Option<u64>,
        items: Vec<ListItem>,
    },
    Quote(Vec<ReplyBlock>),
    Rule,
    Table(Vec<Vec<Vec<Span>>>),
}

/// Item de lista. `task` e o estado da tarefa ("- [x]" e `Some(true)`, "- [ ]" e `Some(false)`);
/// `None` e item comum. Sem ele a interface nao distingue tarefa feita de item solto.
#[allow(dead_code)] // usado na Tarefa 30
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListItem {
    pub task: Option<bool>,
    pub blocks: Vec<ReplyBlock>,
}

#[allow(dead_code)] // usado na Tarefa 30
type Events<'a> = std::iter::Peekable<pulldown_cmark::Parser<'a>>;

/// Interpreta o Markdown da resposta (as mesmas opcoes da mascara: tabela, riscado, tarefas).
#[allow(dead_code)] // usado na Tarefa 30
pub fn parse_reply(content: &str) -> Vec<ReplyBlock> {
    use pulldown_cmark::{Options, Parser};
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut events = Parser::new_ext(content, options).peekable();
    blocks(&mut events)
}

#[allow(dead_code)] // usado na Tarefa 30
fn push_span(spans: &mut Vec<Span>, text: &str, bold: bool, italic: bool, code: bool) {
    if !text.is_empty() {
        spans.push(Span {
            text: text.to_string(),
            bold,
            italic,
            code,
        });
    }
}

/// Le eventos ate o fim do conteiner atual (o `End` que o fecha) ou do texto.
#[allow(dead_code)] // usado na Tarefa 30
fn blocks(events: &mut Events<'_>) -> Vec<ReplyBlock> {
    use pulldown_cmark::{Event, Tag, TagEnd};
    let mut out = Vec::new();
    let mut spans: Vec<Span> = Vec::new();
    let (mut bold, mut italic) = (false, false);
    let mut heading: Option<u8> = None;
    // Item de lista "apertado" nao tem Paragraph: o texto vem solto, e fecha no proximo bloco.
    let flush = |spans: &mut Vec<Span>, heading: &mut Option<u8>, out: &mut Vec<ReplyBlock>| {
        if spans.is_empty() {
            *heading = None;
            return;
        }
        let taken = std::mem::take(spans);
        out.push(match heading.take() {
            Some(level) => ReplyBlock::Heading {
                level,
                spans: taken,
            },
            None => ReplyBlock::Paragraph(taken),
        });
    };
    while let Some(event) = events.next() {
        match event {
            Event::Text(t) => push_span(&mut spans, &t, bold, italic, false),
            Event::Code(t) => push_span(&mut spans, &t, bold, italic, true),
            Event::SoftBreak => push_span(&mut spans, " ", bold, italic, false),
            Event::HardBreak => push_span(&mut spans, "\n", bold, italic, false),
            Event::Start(Tag::Strong) => bold = true,
            Event::Start(Tag::Emphasis) => italic = true,
            Event::End(TagEnd::Strong) => bold = false,
            Event::End(TagEnd::Emphasis) => italic = false,
            Event::Start(Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. })
            | Event::End(TagEnd::Strikethrough | TagEnd::Link | TagEnd::Image) => {}
            Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) => {
                flush(&mut spans, &mut heading, &mut out);
            }
            Event::Start(Tag::Heading { level, .. }) => {
                flush(&mut spans, &mut heading, &mut out);
                heading = Some(level as u8);
            }
            Event::End(TagEnd::Heading(_)) => flush(&mut spans, &mut heading, &mut out),
            Event::Start(Tag::CodeBlock(_)) => {
                flush(&mut spans, &mut heading, &mut out);
                let mut code = String::new();
                for e in events.by_ref() {
                    match e {
                        Event::Text(t) => code.push_str(&t),
                        Event::End(_) => break,
                        _ => {}
                    }
                }
                out.push(ReplyBlock::CodeBlock(
                    code.trim_end_matches('\n')
                        .split('\n')
                        .map(str::to_string)
                        .collect(),
                ));
            }
            Event::Start(Tag::List(first)) => {
                flush(&mut spans, &mut heading, &mut out);
                let mut items = Vec::new();
                while let Some(e) = events.next() {
                    match e {
                        Event::Start(Tag::Item) => {
                            // O marcador vem logo apos o `Start(Item)`, antes de qualquer texto.
                            // Em lista frouxa (itens separados por linha em branco) o marcador vem
                            // depois de `Start(Paragraph)`; consumi-lo e inocuo (so faz flush de
                            // spans vazios) e deixa o marcador visivel.
                            if matches!(events.peek(), Some(Event::Start(Tag::Paragraph))) {
                                events.next();
                            }
                            let task = match events.peek() {
                                Some(Event::TaskListMarker(done)) => Some(*done),
                                _ => None,
                            };
                            if task.is_some() {
                                events.next();
                            }
                            items.push(ListItem {
                                task,
                                blocks: blocks(events),
                            });
                        }
                        Event::End(_) => break,
                        _ => {}
                    }
                }
                out.push(ReplyBlock::List { first, items });
            }
            Event::Start(Tag::BlockQuote(_)) => {
                flush(&mut spans, &mut heading, &mut out);
                out.push(ReplyBlock::Quote(blocks(events)));
            }
            Event::Start(Tag::Table(_)) => {
                flush(&mut spans, &mut heading, &mut out);
                out.push(ReplyBlock::Table(table(events)));
            }
            Event::Rule => {
                flush(&mut spans, &mut heading, &mut out);
                out.push(ReplyBlock::Rule);
            }
            // Bloco HTML: a tela nao o desenha. Consome ate o fim dele, senao o `End` iria cair no
            // catch-all abaixo e fechar um conteiner que esta chamada nunca abriu.
            Event::Start(Tag::HtmlBlock) => {
                for e in events.by_ref() {
                    if matches!(e, Event::End(TagEnd::HtmlBlock)) {
                        break;
                    }
                }
            }
            // Fim do conteiner que esta chamada abriu.
            Event::End(_) => break,
            _ => {}
        }
    }
    flush(&mut spans, &mut heading, &mut out);
    out
}

/// Linhas e celulas da tabela, ate o `End(Table)`.
#[allow(dead_code)] // usado na Tarefa 30
fn table(events: &mut Events<'_>) -> Vec<Vec<Vec<Span>>> {
    use pulldown_cmark::{Event, Tag, TagEnd};
    let mut rows: Vec<Vec<Vec<Span>>> = Vec::new();
    let mut row: Vec<Vec<Span>> = Vec::new();
    let mut cell: Option<Vec<Span>> = None;
    let (mut bold, mut italic) = (false, false);
    for event in events.by_ref() {
        match event {
            Event::Start(Tag::TableRow) => row = Vec::new(),
            Event::Start(Tag::TableCell) => cell = Some(Vec::new()),
            Event::Start(Tag::Strong) => bold = true,
            Event::Start(Tag::Emphasis) => italic = true,
            Event::End(TagEnd::Strong) => bold = false,
            Event::End(TagEnd::Emphasis) => italic = false,
            Event::Text(t) => {
                if let Some(c) = cell.as_mut() {
                    push_span(c, &t, bold, italic, false);
                }
            }
            Event::Code(t) => {
                if let Some(c) = cell.as_mut() {
                    push_span(c, &t, bold, italic, true);
                }
            }
            Event::End(TagEnd::TableCell) => row.push(cell.take().unwrap_or_default()),
            Event::End(TagEnd::TableRow | TagEnd::TableHead) => {
                if !row.is_empty() {
                    rows.push(std::mem::take(&mut row));
                }
            }
            Event::End(TagEnd::Table) => break,
            _ => {}
        }
    }
    rows
}

/// O texto que os blocos desenham, um por linha de bloco. `None` com tabela: a mascara nao
/// confere celulas (o desenho antigo nao as expunha), entao a resposta cai em texto puro.
#[allow(dead_code)] // usado na Tarefa 30
pub fn blocks_text(blocks: &[ReplyBlock]) -> Option<String> {
    fn spans_text(spans: &[Span], out: &mut String) {
        for s in spans {
            out.push_str(&s.text);
        }
        out.push('\n');
    }
    fn walk(blocks: &[ReplyBlock], out: &mut String) -> Option<()> {
        for b in blocks {
            match b {
                ReplyBlock::Heading { spans, .. } | ReplyBlock::Paragraph(spans) => {
                    spans_text(spans, out);
                }
                ReplyBlock::CodeBlock(lines) => {
                    for l in lines {
                        out.push_str(l);
                        out.push('\n');
                    }
                }
                ReplyBlock::List { items, .. } => {
                    for item in items {
                        walk(&item.blocks, out)?;
                    }
                }
                ReplyBlock::Quote(inner) => walk(inner, out)?,
                ReplyBlock::Rule => {}
                ReplyBlock::Table(_) => return None,
            }
        }
        Some(())
    }
    let mut out = String::new();
    walk(blocks, &mut out)?;
    Some(out)
}

/// Como a resposta aparece: blocos, ou texto puro quando a mascara nao pode garantir os blocos.
#[allow(dead_code)] // usado na Tarefa 30
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplyView {
    Blocks(Vec<ReplyBlock>),
    Plain(String),
}

/// Resposta pronta para desenhar e o que "Copiar" leva (com o olho ligado, o texto ja mascarado).
#[allow(dead_code)] // usado na Tarefa 30
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaskedReply {
    pub shown: ReplyView,
    pub copy: String,
}

/// Mesma regra de seguranca da tela de hoje, sobre os blocos proprios: com "Ocultar valores" a
/// mascara roda no fonte **antes** do parse e o resultado e conferido **depois**: o texto
/// desenhado tem de ser exatamente a mascara do que o Markdown original desenharia. Isso pega o
/// que o parse remonta e a mascara do fonte nao ve ("1.**234**,56", "1\.234,56"); nesses casos, e
/// com tabela, sai o texto puro, mascarado depois de interpretado.
#[allow(dead_code)] // usado na Tarefa 30
pub fn masked_reply(content: &str, hidden: bool) -> MaskedReply {
    if !hidden {
        return MaskedReply {
            shown: ReplyView::Blocks(parse_reply(content)),
            copy: content.to_string(),
        };
    }
    let raw = parse_reply(content);
    let masked = parse_reply(&mask_str(content));
    match (blocks_text(&raw), blocks_text(&masked)) {
        (Some(raw_text), Some(masked_text)) => {
            let expected = mask_str(&raw_text);
            let copy = expected.trim_end().to_string();
            if masked_text == expected {
                MaskedReply {
                    shown: ReplyView::Blocks(masked),
                    copy,
                }
            } else {
                MaskedReply {
                    shown: ReplyView::Plain(copy.clone()),
                    copy,
                }
            }
        }
        _ => {
            let plain = mask_str(&plain_text(content)).trim_end().to_string();
            MaskedReply {
                shown: ReplyView::Plain(plain.clone()),
                copy: plain,
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::assistant::view::MessageRole;
    use crate::ui_model::assistant_setup;

    pub(crate) fn conversation_with(messages: Vec<MessageView>) -> OpenConversation {
        OpenConversation {
            id: "C1".into(),
            book: crate::dashboard::contract::Book::Personal,
            envelope: String::new(),
            messages,
            needs_cloud_consent: true,
        }
    }

    pub(crate) fn msg(role: MessageRole, tokens: Option<u32>) -> MessageView {
        MessageView {
            id: "m".into(),
            role,
            content: String::new(),
            sent_to_cloud: false,
            deny_data_collection: None,
            provider: None,
            model: None,
            tokens_in: tokens,
            tokens_out: tokens,
            interrupted: false,
        }
    }

    #[test]
    fn mascara_troca_valores_e_mantem_o_resto() {
        assert_eq!(
            mask_str("Gasto R$ 1.234,56 de R$ 2.000,00 (passou R$ 0,00)."),
            format!("Gasto {MASK} de {MASK} (passou {MASK}).")
        );
    }

    #[test]
    fn mascara_tira_o_sinal_de_menos_e_mantem_o_til() {
        assert_eq!(
            mask_str("Saldo \u{2212}R$ 5,00 hoje"),
            format!("Saldo {MASK} hoje")
        );
        assert_eq!(
            mask_str("faltar ~R$ 960,00 no fim do mês"),
            format!("faltar ~{MASK} no fim do mês")
        );
    }

    #[test]
    fn mascara_preserva_ponto_final() {
        assert_eq!(mask_str("sobrou R$ 960,00."), format!("sobrou {MASK}."));
    }

    #[test]
    fn mascara_deixa_r_cifrao_sem_digito_como_texto() {
        assert_eq!(mask_str("Fundo R$ extra"), "Fundo R$ extra");
        assert_eq!(
            mask_str("Fundo R$ extra: R$ 5,00"),
            format!("Fundo R$ extra: {MASK}")
        );
    }

    #[test]
    fn barras_dos_achados_leem_gasto_e_referencia_na_ordem_de_cada_regra() {
        use crate::dashboard::{GRAFITE_TODAY, fixtures_grafite};
        let analysis = crate::advisor::analyze(&fixtures_grafite(), GRAFITE_TODAY);
        // Pelo indice da cifra, e nao pelo valor: dois valores iguais (gasto igual a meta)
        // fariam a busca por valor achar o rotulo errado e o teste passar por acaso.
        let money_at = |f: &Finding, i: usize| match f.figures[i].value {
            FigureValue::Money(v) => v,
            ref other => panic!("{:?}: cifra {i} nao e dinheiro: {other:?}", f.kind),
        };
        let index_of = |f: &Finding, pick: &dyn Fn(&str) -> bool| {
            let found: Vec<usize> = (0..f.figures.len())
                .filter(|&i| pick(&f.figures[i].label))
                .collect();
            assert_eq!(found.len(), 1, "{:?}: {:?}", f.kind, f.figures);
            found[0]
        };
        let mut seen = 0;
        for f in &analysis.findings {
            let Some(c) = comparison(f) else {
                assert!(
                    !matches!(
                        f.kind,
                        FindingKind::Pace | FindingKind::CategoryAboveNormal | FindingKind::Budget
                    ),
                    "{:?}",
                    f.kind
                );
                continue;
            };
            seen += 1;
            // O indice do gasto e o da cifra cujo rotulo fala de gasto, pelo texto da regra.
            let (real, reference): (usize, usize) = match f.kind {
                FindingKind::Pace => (
                    index_of(f, &|l| l == "Gasto até hoje"),
                    index_of(f, &|l| l == "Normal até hoje"),
                ),
                FindingKind::CategoryAboveNormal => (
                    index_of(f, &|l| l.ends_with(" no mês")),
                    index_of(f, &|l| l.ends_with(" normal")),
                ),
                FindingKind::Budget => {
                    assert_eq!(c.reference_label, "Meta");
                    (
                        index_of(f, &|l| l.starts_with("Gasto ")),
                        index_of(f, &|l| l.starts_with("Meta ")),
                    )
                }
                other => panic!("{other:?}"),
            };
            assert_eq!(c.real, money_at(f, real), "{:?}", f.kind);
            assert_eq!(c.reference, money_at(f, reference), "{:?}", f.kind);
        }
        assert!(seen >= 3, "a fixture tem ritmo, categoria e metas");
    }

    #[test]
    fn mascara_nao_deixa_valor_nenhum_nos_achados_nem_na_resposta() {
        use crate::dashboard::money::leaks;
        use crate::dashboard::{GRAFITE_TODAY, fixtures_grafite};
        let analysis = crate::advisor::analyze(&fixtures_grafite(), GRAFITE_TODAY);
        let mut texts: Vec<&str> = analysis
            .findings
            .iter()
            .flat_map(|f| {
                [
                    Some(f.title.as_str()),
                    Some(f.detail.as_str()),
                    f.action.as_deref(),
                ]
            })
            .flatten()
            .collect();
        assert!(
            texts.iter().any(|t| leaks(t).is_some()),
            "o teste so vale se algum texto trouxer dinheiro"
        );
        // Resposta do modelo sem "R$": o prompt pede o formato, mas a mascara nao confia nele.
        let reply = "Você gastou 1.500 reais em mercado e faltam 5.500,00 para a meta; \
                     no mês passado foram 1500,00 e o saldo ficou -R$ 3,00.";
        texts.push(reply);
        for t in texts {
            let masked = mask_str(t);
            assert_eq!(leaks(&masked), None, "{t:?} -> {masked:?}");
        }
        assert_eq!(
            mask_str(reply),
            format!(
                "Você gastou {MASK_DIGITS} reais em mercado e faltam {MASK_DIGITS} para a meta; \
                 no mês passado foram {MASK_DIGITS} e o saldo ficou {MASK}."
            )
        );
    }

    #[test]
    fn mascara_tira_o_hifen_antes_do_valor_so_quando_e_sinal() {
        assert_eq!(mask_str("saldo -R$ 3,00"), format!("saldo {MASK}"));
        assert_eq!(mask_str("saldo -1.500"), format!("saldo {MASK_DIGITS}"));
        assert_eq!(mask_str("item-1.500"), format!("item-{MASK_DIGITS}"));
    }

    #[test]
    fn mascara_deixa_numeros_que_nao_sao_dinheiro() {
        for t in [
            "Você guarda 80% da renda",
            "cobre só 0,5 mês",
            "subiu 12,5% no mês",
            "subiu 150% no ano",
            "3 meses, dia 24/09",
            "fechou em 19 nov",
            "A1234",
            "5 kg de arroz e 5 km",
            "5 mililitros",
        ] {
            assert_eq!(mask_str(t), t);
        }
        // Um ano tem 4 digitos e cai na regra do inteiro longo: aceito, vazar valor e pior.
        assert_eq!(
            mask_str("subiu 12,5% em 2026"),
            format!("subiu 12,5% em {MASK_DIGITS}")
        );
    }

    /// Os formatos que o modelo usa sem "R$": cada um vira mascara, sem digito nenhum sobrando
    /// (o `leaks` so ve "d,dd" e "R$ d", entao o texto exato e conferido tambem).
    #[test]
    fn mascara_pega_valor_sem_cifrao_em_qualquer_formato() {
        use crate::dashboard::money::leaks;
        let d = MASK_DIGITS;
        let cases = [
            ("gastou 1500 reais", format!("gastou {d} reais")),
            ("gastou 3 reais", format!("gastou {d} reais")),
            ("1 real a mais", format!("{d} real a mais")),
            ("sobram 5 mil", format!("sobram {d}")),
            ("sobram 1,5 mil.", format!("sobram {d}.")),
            ("sobram 5k no mês", format!("sobram {d} no mês")),
            ("perto de 2 milhões", format!("perto de {d}")),
            ("total 1500 no mês", format!("total {d} no mês")),
            ("total 1234.56", format!("total {d}")),
            ("total 1500,5 hoje", format!("total {d} hoje")),
            ("total 1 234,56 hoje", format!("total {d} hoje")),
            ("total 1\u{a0}234,56", format!("total {d}")),
            ("total 1\u{202f}234,56", format!("total {d}")),
            ("total 12,50 hoje", format!("total {d} hoje")),
            ("saldo +1.500", format!("saldo {d}")),
            ("saldo +R$ 3,00", format!("saldo {MASK}")),
            ("R$ 5 mil no mês", format!("{MASK} no mês")),
            ("R$ 1 234,56.", format!("{MASK}.")),
        ];
        for (input, expected) in cases {
            let masked = mask_str(input);
            assert_eq!(masked, expected, "{input:?}");
            assert_eq!(leaks(&masked), None, "{input:?} -> {masked:?}");
            assert!(
                !masked.chars().any(|c| c.is_ascii_digit()),
                "{input:?} -> {masked:?}"
            );
        }
    }

    #[test]
    fn mascara_nao_atravessa_quebra_de_linha() {
        assert_eq!(mask_str("Fundo R$\n1. Mercado"), "Fundo R$\n1. Mercado");
        assert_eq!(mask_str("meses: 3\nReal mesmo"), "meses: 3\nReal mesmo");
        assert_eq!(mask_str("faltam 5\nmil coisas"), "faltam 5\nmil coisas");
        assert_eq!(mask_str("subiu 12,5\n% do total"), "subiu 12,5\n% do total");
    }

    #[test]
    fn mascara_apara_todo_espaco_unicode_depois_do_cifrao() {
        use crate::dashboard::money::leaks;
        for space in [" ", "\u{a0}", "\u{202f}", "\t", "\u{2009}", "  "] {
            let input = format!("custou R${space}1.234,56 hoje");
            let masked = mask_str(&input);
            assert_eq!(masked, format!("custou {MASK} hoje"), "{input:?}");
            assert_eq!(leaks(&masked), None, "{input:?}");
        }
    }

    #[test]
    fn selo_da_nuvem_diz_a_politica_do_envio() {
        assert_eq!(cloud_badge(Some(true)), "enviado à nuvem · sem retenção");
        assert_eq!(
            cloud_badge(Some(false)),
            "enviado à nuvem · retenção permitida"
        );
        assert_eq!(cloud_badge(None), "enviado à nuvem");
    }

    #[test]
    fn rodape_da_resposta_mostra_o_modelo_local_antes_e_o_da_nuvem_depois() {
        let mut m = MessageView {
            id: "m".into(),
            role: MessageRole::Assistant,
            content: String::new(),
            sent_to_cloud: false,
            deny_data_collection: None,
            provider: Some("local".into()),
            model: Some("llama3.1:8b".into()),
            tokens_in: Some(1240),
            tokens_out: Some(380),
            interrupted: false,
        };
        assert_eq!(
            reply_notes(&m).join(" · "),
            "llama3.1:8b · 1.240 → 380 tokens"
        );
        m.sent_to_cloud = true;
        m.provider = Some("cloud".into());
        m.model = Some("openai/gpt-4o".into());
        m.deny_data_collection = Some(true);
        assert_eq!(
            reply_notes(&m).join(" · "),
            "1.240 → 380 tokens · enviado à nuvem · sem retenção · openai/gpt-4o"
        );
        m.interrupted = true;
        assert!(reply_notes(&m).contains(&"interrompida".to_string()));
        // Registro antigo, sem modelo: nada de "None" na tela.
        m.model = None;
        m.provider = None;
        assert_eq!(
            reply_notes(&m).join(" · "),
            "1.240 → 380 tokens · interrompida · enviado à nuvem · sem retenção"
        );
    }

    #[test]
    fn total_do_cabecalho_agrupa_milhares_e_some_sem_uso_informado() {
        assert_eq!(tokens_label((1240, 380)), "1.240 → 380 tokens");
        assert_eq!(tokens_label((1_234_567, 0)), "1.234.567 → 0 tokens");
        let none = conversation_with(vec![
            msg(MessageRole::User, None),
            msg(MessageRole::Assistant, None),
        ]);
        assert_eq!(tokens_total_label(&none), "—");
        let some = conversation_with(vec![
            msg(MessageRole::User, None),
            msg(MessageRole::Assistant, Some(1500)),
        ]);
        assert_eq!(tokens_total_label(&some), "1.500 → 1.500 tokens");
        // Uso informado como zero e uma medida, nao ausencia.
        let zero = conversation_with(vec![msg(MessageRole::Assistant, Some(0))]);
        assert_eq!(tokens_total_label(&zero), "0 → 0 tokens");
    }

    #[test]
    fn linha_de_retencao_da_previa() {
        assert_eq!(retention_value(true), "não permitida");
        assert_eq!(retention_value(false), "permitida");
    }

    #[test]
    fn textos_da_nuvem_nao_tem_espaco_duplo() {
        for text in [
            PREVIEW_INTRO,
            PREVIEW_INTRO_EARLIER,
            assistant_setup::CLOUD_EXPLAIN,
            assistant_setup::DENY_LABEL,
            assistant_setup::DENY_WARNING,
            assistant_setup::PLAIN_KEY_WARNING,
        ] {
            assert!(!text.contains("  "), "{text:?}");
        }
        assert!(
            assistant_setup::CLOUD_EXPLAIN.starts_with(
                "Ligado, o resumo de achados e totais desta conta vai para o OpenRouter"
            )
        );
    }

    /// Lista "Baixo" do relatorio de 25/11: programa de mesa, comando sem crase literal e o
    /// destino no botao de envio.
    #[test]
    fn textos_do_assistente_falam_de_programa_de_mesa() {
        use super::super::assistant_setup::{MODELS_NOT_LISTED, NO_MODELS_HINT};
        for text in [MODELS_NOT_LISTED, NO_MODELS_HINT, PREVIEW_SEND] {
            assert!(!text.contains("  "), "{text:?}");
            assert!(!text.contains('`'), "{text:?}");
            assert!(!text.contains("Toque"), "{text:?}");
        }
        assert!(MODELS_NOT_LISTED.starts_with("Clique em Testar conexão"));
        assert!(NO_MODELS_HINT.contains("ollama pull llama3.1:8b no terminal"));
        assert_eq!(PREVIEW_SEND, "Enviar ao OpenRouter");
    }

    #[test]
    fn texto_da_previa_nao_tem_espaco_duplo() {
        assert!(!INTRO.contains("  "), "{INTRO:?}");
        assert!(INTRO.contains("sem lançamentos, descrições"));
    }

    #[test]
    fn linha_resumo_conta_por_gravidade_no_singular_e_no_plural() {
        use crate::advisor::FindingKind;
        let f = |severity| Finding {
            kind: FindingKind::Pace,
            severity,
            title: String::new(),
            detail: String::new(),
            action: None,
            figures: Vec::new(),
            impact_minor: 0,
        };
        let many = vec![
            f(Severity::Alert),
            f(Severity::Alert),
            f(Severity::Attention),
            f(Severity::Attention),
            f(Severity::Attention),
            f(Severity::GoodSign),
        ];
        assert_eq!(
            summary_line(&many),
            "2 alertas, 3 atenções, 1 bom sinal. Calculado no seu PC."
        );
        let few = vec![
            f(Severity::Attention),
            f(Severity::GoodSign),
            f(Severity::GoodSign),
        ];
        assert_eq!(
            summary_line(&few),
            "1 atenção, 2 bons sinais. Calculado no seu PC."
        );
        assert_eq!(summary_line(&[]), "Nada a apontar. Calculado no seu PC.");
    }

    #[test]
    fn texto_chegando_esconde_o_numero_pela_metade_no_fim() {
        use crate::dashboard::money::leaks;
        for partial in ["gastou 1500,0", "gastou 1.23", "saldo 1.234,5"] {
            let shown = mask_streaming(partial);
            assert!(
                !shown.bytes().any(|b| b.is_ascii_digit()),
                "{partial:?} -> {shown:?}"
            );
        }
        assert_eq!(mask_streaming("gastou 1500,0"), "gastou ");
        // Numero terminado (com texto depois) segue a mascara de sempre; o resto fica.
        let done = mask_streaming("gastou R$ 1.500,00 em mercado e 3");
        assert_eq!(leaks(&done), None);
        assert!(
            done.starts_with(&format!("gastou {MASK} em mercado e ")),
            "{done}"
        );
        assert_eq!(mask_streaming("tudo certo."), "tudo certo.");
    }

    /// O streaming mascara o texto desenhado (`plain_text` do parcial): a cada pedaco que chega,
    /// nenhum digito de valor aparece, nem quando a unidade, o milhar ou a enfase ainda nao
    /// chegaram.
    #[test]
    fn texto_chegando_nao_mostra_digito_em_prefixo_nenhum() {
        use crate::dashboard::money::leaks;
        let full = [
            "gastou 5 mil no mês",
            "sobram 1,5 mil hoje",
            "total 1 234,56 hoje",
            "foi R$ **1500** no mês",
            "foi 1.**234**,56 hoje",
            "foi R\\$ 1.500 hoje",
            "saldo -1.500 e +R$ 3,00 ok",
        ];
        for text in full {
            for (i, _) in text.char_indices().skip(1).chain([(text.len(), ' ')]) {
                let partial = &text[..i];
                let shown = mask_streaming(&plain_text(partial));
                assert!(
                    !shown.chars().any(|c| c.is_ascii_digit()),
                    "{partial:?} -> {shown:?}"
                );
                assert_eq!(leaks(&shown), None, "{partial:?} -> {shown:?}");
            }
        }
        // Terminados, os tres casos de Markdown viram a mascara exata.
        assert_eq!(
            mask_streaming(&plain_text("foi R$ **1500** no mês")),
            format!("foi {MASK} no mês")
        );
        assert_eq!(
            mask_streaming(&plain_text("foi 1.**234**,56 hoje")),
            format!("foi {MASK_DIGITS} hoje")
        );
        assert_eq!(
            mask_streaming(&plain_text("foi R\\$ 1.500 hoje")),
            format!("foi {MASK} hoje")
        );
    }

    #[test]
    fn mascara_nao_mexe_em_texto_sem_dinheiro() {
        let s = "Gasto 35% acima do normal em 12 dias.";
        assert_eq!(mask_str(s), s);
        assert_eq!(mask_str(""), "");
    }

    #[test]
    fn resposta_vira_blocos_com_enfase_lista_e_codigo() {
        let src = "# Titulo\n\nTexto **forte** e *leve* com `cod`.\n\n- um\n- dois\n\n```\na\nb\n```\n\n---\n";
        let blocks = parse_reply(src);
        assert_eq!(blocks.len(), 5, "{blocks:?}");
        assert!(matches!(&blocks[0], ReplyBlock::Heading { level: 1, .. }));
        let ReplyBlock::Paragraph(spans) = &blocks[1] else {
            panic!("{blocks:?}")
        };
        assert!(
            spans
                .iter()
                .any(|s| s.text == "forte" && s.bold && !s.italic)
        );
        assert!(
            spans
                .iter()
                .any(|s| s.text == "leve" && s.italic && !s.bold)
        );
        assert!(spans.iter().any(|s| s.text == "cod" && s.code));
        let ReplyBlock::List { first: None, items } = &blocks[2] else {
            panic!("{blocks:?}")
        };
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.task.is_none()));
        assert_eq!(
            blocks[3],
            ReplyBlock::CodeBlock(vec!["a".into(), "b".into()])
        );
        assert_eq!(blocks[4], ReplyBlock::Rule);
    }

    #[test]
    fn lista_de_tarefas_guarda_o_estado_de_cada_item() {
        let b = parse_reply(
            "- [x] feito
- [ ] falta
- comum
",
        );
        let [ReplyBlock::List { items, .. }] = b.as_slice() else {
            panic!("{b:?}")
        };
        let tasks: Vec<Option<bool>> = items.iter().map(|i| i.task).collect();
        assert_eq!(tasks, vec![Some(true), Some(false), None]);
        let text = blocks_text(&b).expect("sem tabela");
        assert!(text.contains("feito") && text.contains("falta"), "{text:?}");
    }

    #[test]
    fn lista_frouxa_de_tarefas_guarda_o_estado() {
        let b = parse_reply(
            "- [x] a

- [ ] b
",
        );
        let [ReplyBlock::List { items, .. }] = b.as_slice() else {
            panic!("{b:?}")
        };
        let tasks: Vec<Option<bool>> = items.iter().map(|i| i.task).collect();
        assert_eq!(tasks, vec![Some(true), Some(false)]);
    }

    #[test]
    fn tabela_guarda_a_enfase_das_celulas() {
        let b = parse_reply(
            "| x |
|---|
| **forte** |
",
        );
        let [ReplyBlock::Table(rows)] = b.as_slice() else {
            panic!("{b:?}")
        };
        let cell = &rows[1][0];
        assert!(
            cell.iter().any(|sp| sp.text == "forte" && sp.bold),
            "{cell:?}"
        );
    }

    #[test]
    fn resposta_com_valores_a_mostra_fica_como_veio() {
        let r = masked_reply("Gastou **R$ 1.234,56** em mercado.", false);
        assert_eq!(r.copy, "Gastou **R$ 1.234,56** em mercado.");
        let ReplyView::Blocks(b) = r.shown else {
            panic!("esperava blocos")
        };
        assert_eq!(
            blocks_text(&b).as_deref(),
            Some("Gastou R$ 1.234,56 em mercado.\n")
        );
    }

    #[test]
    fn resposta_oculta_nao_deixa_digito_nos_blocos_nem_na_copia() {
        let r = masked_reply("Gastou **R$ 1.234,56** em mercado.", true);
        let text = match &r.shown {
            ReplyView::Blocks(b) => blocks_text(b).unwrap(),
            ReplyView::Plain(p) => p.clone(),
        };
        assert!(text.contains(MASK), "{text}");
        assert!(!text.chars().any(|c| c.is_ascii_digit()), "{text}");
        assert!(!r.copy.chars().any(|c| c.is_ascii_digit()), "{}", r.copy);
    }

    #[test]
    fn numero_remontado_pelo_markdown_cai_em_texto_puro_mascarado() {
        for src in ["valor 1.**234**,56 hoje", "valor 1\\.234,56 hoje"] {
            let r = masked_reply(src, true);
            let ReplyView::Plain(p) = &r.shown else {
                panic!("{src}: esperava texto puro, veio {:?}", r.shown)
            };
            assert!(!p.chars().any(|c| c.is_ascii_digit()), "{src} -> {p}");
            assert_eq!(&r.copy, p);
        }
    }

    #[test]
    fn tabela_oculta_sai_em_texto_puro() {
        let r = masked_reply("| a | b |\n|---|---|\n| R$ 1.500,00 | 2 |\n", true);
        assert!(matches!(r.shown, ReplyView::Plain(_)), "{:?}", r.shown);
        assert!(!r.copy.contains("1.500"), "{}", r.copy);
        // Com os valores a mostra, a tabela e um bloco.
        let r = masked_reply("| a | b |\n|---|---|\n| 1 | 2 |\n", false);
        let ReplyView::Blocks(b) = r.shown else {
            panic!("esperava blocos")
        };
        assert!(matches!(&b[0], ReplyBlock::Table(rows) if rows.len() == 2));
    }

    /// Bloco HTML no nivel de cima e dentro de item de lista: o `End(HtmlBlock)` nao pode fechar
    /// um conteiner que a chamada nao abriu (o resto da resposta sumia calado).
    #[test]
    fn bloco_html_nao_corta_o_resto_da_resposta() {
        let b = parse_reply(
            "Antes do bloco.\n\n<br>\n\nDepois: R$ 1.234,56\n\n- item um\n- item dois\n",
        );
        let text = blocks_text(&b).expect("sem tabela");
        assert!(text.contains("Antes do bloco."), "{text:?}");
        assert!(text.contains("Depois: R$ 1.234,56"), "{text:?}");
        assert!(
            text.contains("item um") && text.contains("item dois"),
            "{text:?}"
        );
        assert!(!text.contains("<br>"), "{text:?}");

        let b = parse_reply("- a\n\n  <div>x</div>\n\n- b\n\nfim\n");
        let [ReplyBlock::List { items, .. }, ReplyBlock::Paragraph(_)] = b.as_slice() else {
            panic!("esperava lista e paragrafo: {b:?}");
        };
        assert_eq!(items.len(), 2, "{items:?}");
        let text = blocks_text(&b).expect("sem tabela");
        assert!(
            text.contains('a') && text.contains('b') && text.contains("fim"),
            "{text:?}"
        );

        let m = masked_reply("Antes.\n\n<br>\n\nDepois: R$ 1.234,56\n\n- um\n", true);
        assert!(
            m.copy.contains("Depois") && m.copy.contains("um"),
            "{}",
            m.copy
        );
        assert_eq!(crate::dashboard::money::leaks(&m.copy), None, "{}", m.copy);
    }

    /// Respostas com dinheiro em Markdown: o que `masked_reply` entrega para desenhar (blocos ou
    /// texto puro) e para copiar nunca deixa valor, nem quando a marcacao corta o numero.
    #[test]
    fn markdown_mascarado_nao_vaza_valor() {
        use crate::dashboard::money::leaks;
        let cases = [
            "**Mercado:** R$ 1.234,56 em novembro.",
            "1. Delivery: *R$ 612,00*\n2. Mercado: `R$ 980,10`\n",
            "# Resumo\n\n- saldo -R$ 3,00\n- meta de 5.500,00\n\n> sobram 1.500 reais",
            "Valor 1.**234**,56 com enfase no meio.",
            "Escapado 1\x5c.234,56 no texto.",
            "Entidade &#49;.234,56 no texto.",
            "```\ntotal R$ 9.876,54\n```",
            "[R$ 10,00](https://exemplo.invalid) de link e ![R$ 20,00](img.png)",
            "| Categoria | Valor |\n|---|---|\n| Mercado | R$ 1.**234**,56 |",
            "| x | y |\n|---|---|\n| x | 1.~~234~~,56 |",
            "| x | y |\n|---|---|\n| x | 1.<b>234</b>,56 |",
            "| x | y |\n|---|---|\n| a | b |\n\nTexto 1.<b>234</b>,56 fora",
            "| x | y |\n|---|---|\n| x | 1\x5c.234,56 |",
            "Texto 1.<b>234</b>,56 inline",
            "Antes.\n\n<br>\n\nDepois R$ 1.234,56\n\n- um 2.500,00\n",
        ];
        for case in cases {
            let r = masked_reply(case, true);
            let shown = match &r.shown {
                ReplyView::Blocks(b) => blocks_text(b).expect("sem tabela"),
                ReplyView::Plain(s) => s.clone(),
            };
            assert_eq!(leaks(&shown), None, "{case:?} -> {shown:?}");
            assert_eq!(mask_str(&shown), shown, "{case:?} -> {shown:?}");
            assert_eq!(leaks(&r.copy), None, "copia: {case:?} -> {:?}", r.copy);
            assert_eq!(mask_str(&r.copy), r.copy, "copia: {case:?}");
            // Quem le junta "1.~~234~~,56" de cabeca: sem as marcas, continua sem valor.
            let unmarked: String = shown
                .replace("<b>", "")
                .replace("</b>", "")
                .chars()
                .filter(|c| !matches!(c, '*' | '_' | '`' | '~' | '\x5c'))
                .collect();
            assert_eq!(leaks(&unmarked), None, "{case:?} -> {shown:?}");
            assert_eq!(mask_str(&unmarked), unmarked, "{case:?} -> {shown:?}");
        }
        // A reserva da tabela le as celulas: o valor da celula vira mascara, o rotulo fica.
        let table = masked_reply("| x | y |\n|---|---|\n| Mercado | 1.~~234~~,56 |", true);
        assert!(table.copy.contains("Mercado"), "{}", table.copy);
        assert!(table.copy.contains(MASK_DIGITS), "{}", table.copy);
        assert!(matches!(table.shown, ReplyView::Plain(_)));
        // Os casos comuns continuam em blocos; os tortos caem no texto puro.
        assert!(matches!(
            masked_reply("**Mercado:** R$ 1.234,56", true).shown,
            ReplyView::Blocks(_)
        ));
        assert!(matches!(
            masked_reply("Valor 1.**234**,56", true).shown,
            ReplyView::Plain(_)
        ));
        // Sem ocultar, a copia e o texto inteiro.
        let shown = masked_reply("**R$ 1.234,56**", false);
        assert!(matches!(shown.shown, ReplyView::Blocks(_)));
        assert_eq!(shown.copy, "**R$ 1.234,56**");
    }
}
