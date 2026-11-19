//! Contrato de campos: o que o dashboard le de cada tabela que o app sincroniza.
//!
//! O hub guarda JSON opaco; so aqui uma linha vira transacao, categoria etc. Campo
//! desconhecido e ignorado (sem `deny_unknown_fields`): o app ganha campos novos sem o hub
//! mudar. Campo do contrato ausente ou com tipo errado faz a *linha* ser ignorada e contada
//! pelo `Dataset`, sem derrubar sync nem janela.
//!
//! `#[serde(default)]` so nos campos anulaveis: para campo obrigatorio, ausente nao e nulo.
//! Para anulavel, ausente vale nulo porque o `ROADMAP.md` do mobile fixa que campo novo nasce
//! anulavel justamente para linha antiga continuar valida.

use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Income,
    Expense,
}

/// Estados que tiram um lancamento das contas (`reportState` do app, `transaction.ts`).
///
/// So os dois valores conhecidos excluem; qualquer outro (texto novo de uma versao futura do app,
/// outro tipo JSON) conta como lancamento normal, como `countsInTotals` do app: o erro seguro e
/// somar a mais, nunca fazer o dinheiro sumir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportState {
    /// Movimento entre contas proprias: nao e receita nem despesa.
    Transfer,
    /// Continua na lista, mas fora de qualquer soma.
    Hidden,
}

impl ReportState {
    pub fn from_value(value: &serde_json::Value) -> Option<Self> {
        match value.as_str() {
            Some("transfer") => Some(Self::Transfer),
            Some("hidden") => Some(Self::Hidden),
            _ => None,
        }
    }

    /// Rotulo da tag na lista.
    pub fn label(self) -> &'static str {
        match self {
            Self::Transfer => "Transferência",
            Self::Hidden => "Oculto",
        }
    }
}

/// Conta a que uma linha pertence (`domain/model/book.ts` do app).
///
/// Ausente, `null` e qualquer valor que esta versao nao conhece valem `Personal`, como o
/// `bookOf` do app: linha antiga (de antes da conta comercial) continua sendo pessoal, e um
/// valor de versao futura degrada para a conta de sempre em vez de derrubar a linha. O erro
/// seguro e a linha aparecer na conta pessoal, nunca sumir.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Book {
    #[default]
    Personal,
    Business,
}

impl Book {
    pub fn from_value(value: &serde_json::Value) -> Self {
        match value.as_str() {
            Some("business") => Self::Business,
            _ => Self::Personal,
        }
    }

    /// `Option` porque o campo e anulavel; ver `from_value`.
    pub fn from_field(value: Option<&serde_json::Value>) -> Self {
        value.map_or(Self::Personal, Self::from_value)
    }

    /// Rotulo do seletor da janela.
    pub fn label(self) -> &'static str {
        match self {
            Self::Personal => "Pessoal",
            Self::Business => "Comercial",
        }
    }
}

// ---- Campos novos em tabela existente: leitura em duas etapas ----
//
// Etapa 1: o serde aceita qualquer JSON (`Option<Value>`) e nunca derruba a linha. Etapa 2: uma
// funcao pura interpreta; o que nao se interpreta degrada (a linha continua somando, so deixa de
// ser "projetavel"). Linha de app antigo, sem o campo, tem de continuar exatamente como era.
// `Option<Value>` le `null` e ausente igual (`None`); o app grava esses campos anulaveis, entao
// nos dois casos o significado e o mesmo.

fn str_of(value: Option<&serde_json::Value>) -> Option<&str> {
    value.and_then(serde_json::Value::as_str)
}

/// Inteiro JSON (`12`). `12.5`, `"12"` e `12.0` nao: o app grava com `JSON.stringify`, que
/// escreve `12` para inteiro, entao `12.0` so viria de outro escritor.
fn int_of(value: Option<&serde_json::Value>) -> Option<i64> {
    value.and_then(serde_json::Value::as_i64)
}

fn is_true(value: Option<&serde_json::Value>) -> bool {
    matches!(value, Some(serde_json::Value::Bool(true)))
}

/// `src/domain/model/transaction.ts`. Nao lidos: `currency`, `cashbackMinor`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionFields {
    pub kind: Kind,
    pub description: String,
    /// Inteiro JSON: `12.5` e `"12"` sao tipo errado (o `serde_json` recusa os dois para `i64`).
    pub amount_minor: i64,
    pub occurred_on: String,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub payment_method_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub recurrence_id: Option<String>,
    /// `reportState` do app. `Value` e nao `String` de proposito: campo novo de versao futura,
    /// de qualquer tipo, nao pode fazer a *linha* ser ignorada; ver `ReportState::from_value`.
    #[serde(default)]
    pub report_state: Option<serde_json::Value>,
    /// `book` do app; ver `Book::from_field`. `Value` e nao `Book` para valor desconhecido (ou
    /// de outro tipo JSON) nao fazer a *linha* ser ignorada.
    #[serde(default)]
    pub book: Option<serde_json::Value>,
    /// Ocorrencia de serie "valor varia" ainda nao confirmada. Ver `estimated()`.
    #[serde(default)]
    pub estimated: Option<serde_json::Value>,
    /// `"<recurrenceId>:<periodo>"` da ocorrencia. Ver `occurrence_key()`.
    #[serde(default)]
    pub occurrence_key: Option<serde_json::Value>,
}

impl TransactionFields {
    pub fn book(&self) -> Book {
        Book::from_field(self.book.as_ref())
    }

    pub fn report_state(&self) -> Option<ReportState> {
        self.report_state.as_ref().and_then(ReportState::from_value)
    }

    /// So `true` e estimada (`estimated !== true` do app): linha anterior a coluna, `null` ou
    /// valor de outro tipo e valor real, confirmado.
    pub fn estimated(&self) -> bool {
        is_true(self.estimated.as_ref())
    }

    /// Texto vira chave; qualquer outro tipo e "sem chave" (avulso para a estimativa).
    pub fn occurrence_key(&self) -> Option<&str> {
        str_of(self.occurrence_key.as_ref())
    }
}

/// `src/domain/model/category.ts`. Token de cor e chave de icone desconhecidos sao validos:
/// a tela cai no neutro e no `circle-dashed`, a linha guarda o que veio.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryFields {
    pub name: String,
    pub color: String,
    pub icon: String,
    /// So em tombstone: a fusao dos padroes do app (spec mobile `padroes-estaveis`) apaga cada
    /// copia apontando para a linha estavel, sem regravar os lancamentos antigos. O dashboard
    /// segue o campo ao resolver referencias; o sync nao o conhece.
    #[serde(default)]
    pub merged_into: Option<String>,
    /// `book` do app; ver `Book::from_field`. `Value` e nao `Book` para valor desconhecido (ou
    /// de outro tipo JSON) nao fazer a *linha* ser ignorada.
    #[serde(default)]
    pub book: Option<serde_json::Value>,
}

impl CategoryFields {
    pub fn book(&self) -> Book {
        Book::from_field(self.book.as_ref())
    }
}

/// `src/domain/model/user.ts`. `avatar` e um data URI; decodificar e problema da tela.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserFields {
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub avatar: Option<String>,
}

/// `src/domain/model/payment-method.ts`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentMethodFields {
    pub name: String,
    pub icon: String,
    /// Como em `CategoryFields`; so em tombstone: a fusao dos padroes do app (spec mobile `padroes-estaveis`) apaga cada
    /// copia apontando para a linha estavel, sem regravar os lancamentos antigos. O dashboard
    /// segue o campo ao resolver referencias; o sync nao o conhece.
    #[serde(default)]
    pub merged_into: Option<String>,
    /// `book` do app; ver `Book::from_field`. `Value` e nao `Book` para valor desconhecido (ou
    /// de outro tipo JSON) nao fazer a *linha* ser ignorada.
    #[serde(default)]
    pub book: Option<serde_json::Value>,
    /// `cash`, `pix`, `credit`... So `credit` importa (cartao); ver `card()`.
    #[serde(default)]
    pub kind: Option<serde_json::Value>,
    #[serde(default)]
    pub bank: Option<serde_json::Value>,
    #[serde(default)]
    pub closing_day: Option<serde_json::Value>,
    #[serde(default)]
    pub due_day: Option<serde_json::Value>,
}

/// Dados de um cartao completo: o bastante para calcular fatura (`cards/card.ts` do app).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    /// Ja aparado.
    pub bank: String,
    pub closing_day: u8,
    pub due_day: u8,
}

/// `isValidCardDay` do app: inteiro de 1 a 31. Texto, 0, 32 ou decimal de outra versao nao
/// passam: vira "cartao por completar" em vez de uma fatura com data inventada.
pub fn card_day(value: Option<&serde_json::Value>) -> Option<u8> {
    int_of(value)
        .filter(|d| (1..=31).contains(d))
        .and_then(|d| u8::try_from(d).ok())
}

impl PaymentMethodFields {
    pub fn book(&self) -> Book {
        Book::from_field(self.book.as_ref())
    }

    pub fn is_credit(&self) -> bool {
        str_of(self.kind.as_ref()) == Some("credit")
    }

    /// `isCompleteCard` do app: credito, banco nao vazio depois de aparar e os dois dias
    /// validos. Qualquer outro caso e uma forma de pagamento valida sem fatura, nunca linha
    /// ignorada: o "Cartao de credito" generico de antes dos cartoes continua somando.
    pub fn card(&self) -> Option<Card> {
        if !self.is_credit() {
            return None;
        }
        let bank = str_of(self.bank.as_ref())?.trim();
        if bank.is_empty() {
            return None;
        }
        Some(Card {
            bank: bank.to_string(),
            closing_day: card_day(self.closing_day.as_ref())?,
            due_day: card_day(self.due_day.as_ref())?,
        })
    }

    /// So os testes: a tela le `card()` direto.
    #[cfg(test)]
    pub fn is_complete_card(&self) -> bool {
        self.card().is_some()
    }
}

/// `RECURRENCE_FREQUENCIES` do app. Valor fora daqui (versao futura) nao gera ocorrencia: a
/// serie fica parada em vez de o hub inventar um passo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frequency {
    Weekly,
    Biweekly,
    Monthly,
    Bimonthly,
    Quarterly,
    Semiannual,
    Annual,
}

impl Frequency {
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "weekly" => Self::Weekly,
            "biweekly" => Self::Biweekly,
            "monthly" => Self::Monthly,
            "bimonthly" => Self::Bimonthly,
            "quarterly" => Self::Quarterly,
            "semiannual" => Self::Semiannual,
            "annual" => Self::Annual,
            _ => return None,
        })
    }

    /// Semanal e quinzenal andam em dias e a competencia e a propria data (`YYYY-MM-DD`); as
    /// demais andam em meses e a competencia e `YYYY-MM` (`isDayStep` do app).
    pub fn is_day_step(self) -> bool {
        matches!(self, Self::Weekly | Self::Biweekly)
    }
}

/// Dia da ocorrencia dentro da competencia mensal (`scheduleType` + `scheduleN`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Schedule {
    /// Dia civil, limitado ao ultimo do mes (`dayOfMonthClamped`).
    DayOfMonth(u32),
    /// N-esimo dia util (`nthBusinessDayOfMonth`).
    NthBusinessDay(u32),
}

/// A regra de uma serie, so quando tudo o que a projecao precisa e interpretavel (spec 4.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecurrenceRule {
    pub frequency: Frequency,
    /// `None` so em semanal/quinzenal, onde a competencia ja e a data e o app ignora a agenda
    /// do mes (`occurrenceOn`); serie mensal sem agenda valida nao tem regra.
    pub schedule: Option<Schedule>,
    pub kind: Kind,
    pub amount_minor: i64,
    /// Data na forma `YYYY-MM-DD` (`is_iso_date`): e ela que garante que um laco de
    /// `periods::shift_day` a partir daqui avanca.
    pub start_on: String,
    pub end_on: Option<String>,
    pub active: bool,
    pub variable: bool,
    /// So inteiro > 0; senao `None` e a 1a ocorrencia usa `amount_minor`.
    pub first_amount_minor: Option<i64>,
}

/// `src/domain/model/recurrence.ts`. `frequency` continua obrigatorio como sempre foi (e o
/// rotulo da tag); os outros 13 sao etapa 1 (`Option<Value>`) e so `rule()` os interpreta.
/// Nao lidos: `currency`, `cashbackMinor`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurrenceFields {
    pub frequency: String,
    /// `book` do app; ver `Book::from_field`. `Value` e nao `Book` para valor desconhecido (ou
    /// de outro tipo JSON) nao fazer a *linha* ser ignorada.
    #[serde(default)]
    pub book: Option<serde_json::Value>,
    #[serde(default)]
    pub kind: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<serde_json::Value>,
    #[serde(default)]
    pub amount_minor: Option<serde_json::Value>,
    #[serde(default)]
    pub category_id: Option<serde_json::Value>,
    #[serde(default)]
    pub payment_method_id: Option<serde_json::Value>,
    #[serde(default)]
    pub schedule_type: Option<serde_json::Value>,
    #[serde(default)]
    pub schedule_n: Option<serde_json::Value>,
    #[serde(default)]
    pub start_on: Option<serde_json::Value>,
    #[serde(default)]
    pub end_on: Option<serde_json::Value>,
    #[serde(default)]
    pub active: Option<serde_json::Value>,
    #[serde(default)]
    pub variable: Option<serde_json::Value>,
    #[serde(default)]
    pub first_amount_minor: Option<serde_json::Value>,
    #[serde(default)]
    pub user_id: Option<serde_json::Value>,
}

impl RecurrenceFields {
    pub fn book(&self) -> Book {
        Book::from_field(self.book.as_ref())
    }

    /// `"income"` e receita; **qualquer outro** valor e despesa, como `totals`/`recurringMonth`
    /// do app tratam a serie.
    pub fn kind(&self) -> Kind {
        match str_of(self.kind.as_ref()) {
            Some("income") => Kind::Income,
            _ => Kind::Expense,
        }
    }

    pub fn description(&self) -> Option<&str> {
        str_of(self.description.as_ref())
    }

    pub fn category_id(&self) -> Option<&str> {
        str_of(self.category_id.as_ref())
    }

    pub fn payment_method_id(&self) -> Option<&str> {
        str_of(self.payment_method_id.as_ref())
    }

    pub fn user_id(&self) -> Option<&str> {
        str_of(self.user_id.as_ref())
    }

    /// "Valor varia": so `true`. Lido mesmo sem `rule()`, porque fixo x variavel olha a serie
    /// de qualquer lancamento com `recurrenceId`, projetavel ou nao.
    pub fn is_variable(&self) -> bool {
        is_true(self.variable.as_ref())
    }

    /// Etapa 2 (spec 4.2): `None` quando algo que a projecao precisa nao se interpreta. Nao e
    /// "fora do contrato": a linha continua valida, da o rotulo e seus lancamentos somam; a
    /// serie so nao entra em "proximas" nem na previsao.
    pub fn rule(&self) -> Option<RecurrenceRule> {
        let frequency = Frequency::parse(&self.frequency)?;
        let schedule = match (
            str_of(self.schedule_type.as_ref()),
            int_of(self.schedule_n.as_ref())
                .filter(|n| *n >= 1)
                .and_then(|n| u32::try_from(n).ok()),
        ) {
            (Some("dayOfMonth"), Some(n)) => Some(Schedule::DayOfMonth(n)),
            (Some("nthBusinessDay"), Some(n)) => Some(Schedule::NthBusinessDay(n)),
            _ => None,
        };
        if schedule.is_none() && !frequency.is_day_step() {
            return None;
        }
        let start_on = str_of(self.start_on.as_ref()).filter(|d| is_iso_date(d))?;
        let end_on = match self.end_on.as_ref() {
            None => None,
            Some(value) => Some(value.as_str().filter(|d| is_iso_date(d))?.to_string()),
        };
        // [inventada, spec 4.2] ausente (ou `null`, que o serde le igual) e serie ativa: app
        // antigo so gravava serie ativa. Outro tipo invalida a regra.
        let active = match self.active.as_ref() {
            None => true,
            Some(value) => value.as_bool()?,
        };
        Some(RecurrenceRule {
            frequency,
            schedule,
            kind: self.kind(),
            amount_minor: int_of(self.amount_minor.as_ref())?,
            start_on: start_on.to_string(),
            end_on,
            active,
            variable: self.is_variable(),
            first_amount_minor: int_of(self.first_amount_minor.as_ref()).filter(|v| *v > 0),
        })
    }
}

/// Linha de `recurrenceAdjustments` (`recurrence-adjustment.ts` do app): valor novo de uma serie
/// a partir de uma competencia. Tabela nova no contrato, entao os tres campos sao obrigatorios
/// (o app sempre os gravou) e linha sem eles e "fora do contrato", ignorada e contada.
///
/// `fromPeriod` nao tem a forma validada: e `YYYY-MM` nas mensais e `YYYY-MM-DD` nas
/// semanais/quinzenais, e o app so o compara como texto com a competencia (`amountFor`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurrenceAdjustmentFields {
    pub recurrence_id: String,
    pub from_period: String,
    pub amount_minor: i64,
    /// `book` do app; ver `Book::from_field`.
    #[serde(default)]
    pub book: Option<serde_json::Value>,
}

impl RecurrenceAdjustmentFields {
    pub fn book(&self) -> Book {
        Book::from_field(self.book.as_ref())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReserveKind {
    Emergency,
    /// O texto de UI continua "Caixinha"; "goal" e o nome que o app grava
    /// (`HomeFinance_Mobile/src/domain/model/reserve.ts`). Sem alias para "pot": nenhum app
    /// gravou "pot", so testes e seed usavam, e um alias aceitaria dado que o app nunca produz.
    Goal,
}

/// Linha de `reserves`. O contrato segue o formato que o app ja grava
/// (`HomeFinance_Mobile/src/domain/model/reserve.ts`; handoff em
/// `docs/desktop/plans/2026-10-07-hub-reservas-contrato-app.md`): o log e eterno e ja ha
/// linhas gravadas assim, por isso e o hub que se adapta, nao o contrario.
/// Nao lidos: `createdAt` e `recurring.day` (regra de materializacao do app).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveFields {
    pub kind: ReserveKind,
    pub name: String,
    pub icon: String,
    pub color: String,
    /// Meta da caixinha (`targetMinor` no app). Nao e lida para `emergency`: a meta dela e
    /// derivada do custo essencial.
    #[serde(default, rename = "targetMinor")]
    pub goal_minor: Option<i64>,
    /// `u8` de proposito: o serde recusa fracao, texto e fora de 0..=255, entao "tipo errado"
    /// sai de graca, sem validacao a mais. Nulo vale o padrao (6) na tela.
    #[serde(default)]
    pub multiple: Option<u8>,
    /// Anulavel porque e configuracao: nulo vale "nenhuma categoria essencial", e a tela diz
    /// isso em vez de ignorar a reserva inteira.
    #[serde(default)]
    pub essential_category_ids: Option<Vec<String>>,
    /// Custo essencial digitado no app quando nao havia historico; substitui o calculado
    /// (o uso na meta da emergencia e de outra etapa, aqui so se carrega).
    #[serde(default)]
    pub essential_override_minor: Option<i64>,
    /// `deadline` no app: `YYYY-MM`; fora do formato e tipo errado (compara como string com
    /// `month_of`).
    #[serde(default, rename = "deadline")]
    pub due_month: Option<String>,
    /// Deposito mensal. Objeto errado ("sim", `amountMinor` fracionado ou ausente) derruba a
    /// linha pelo proprio serde, como os outros campos: o hub nao adivinha o que o app quis dizer.
    #[serde(default)]
    pub recurring: Option<RecurringFields>,
    /// `book` do app; ver `Book::from_field`. `Value` e nao `Book` para valor desconhecido (ou
    /// de outro tipo JSON) nao fazer a *linha* ser ignorada.
    #[serde(default)]
    pub book: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecurringFields {
    pub amount_minor: i64,
    /// Nao lido pelo hub (regra de materializacao do app); aceito para tipar o objeto.
    #[serde(default)]
    pub day: Option<u8>,
    /// Primeiro mes (`YYYY-MM`) em que o deposito vale.
    #[serde(default)]
    pub since: Option<String>,
}

impl ReserveFields {
    pub fn book(&self) -> Book {
        Book::from_field(self.book.as_ref())
    }

    pub fn recurring_amount_minor(&self) -> Option<i64> {
        self.recurring.as_ref().map(|r| r.amount_minor)
    }

    pub fn recurring_since(&self) -> Option<&str> {
        self.recurring.as_ref().and_then(|r| r.since.as_deref())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovementKind {
    Deposit,
    Withdrawal,
}

/// Linha de `budgets` (`budget.ts` do app): meta mensal de gasto de uma categoria.
///
/// `updatedAt` (HLC) vem no JSON porque o hub guarda a linha como o app a mandou; ele decide
/// qual de duas metas da mesma categoria vale (ver `budgets::current_budgets`). Ausente, a meta
/// perde para qualquer outra.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BudgetFields {
    pub category_id: String,
    pub limit_minor: i64,
    #[serde(default)]
    pub updated_at: Option<String>,
    /// `book` do app; ver `Book::from_field`. `Value` e nao `Book` para valor desconhecido (ou
    /// de outro tipo JSON) nao fazer a *linha* ser ignorada.
    #[serde(default)]
    pub book: Option<serde_json::Value>,
}

impl BudgetFields {
    pub fn book(&self) -> Book {
        Book::from_field(self.book.as_ref())
    }
}

/// Linha de `books` (`book.ts` do app): nome, cor e icone da conta comercial, sincronizados
/// (o `meta` do app e local). So a conta `business` tem linha; a pessoal e implicita.
///
/// `kind` e texto para valor desconhecido nao derrubar a linha: vira `Personal`, que nao abre
/// o seletor (so uma linha `business` viva o abre).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookFields {
    pub kind: String,
    pub name: String,
    pub color: String,
    pub icon: String,
}

impl BookFields {
    pub fn kind(&self) -> Book {
        match self.kind.as_str() {
            "business" => Book::Business,
            _ => Book::Personal,
        }
    }
}

/// Tipo de bem (`assets`). Fechado: tipo desconhecido derruba a linha (ignorada e contada),
/// porque somar um bem que esta versao nao sabe classificar falsearia o capital por tipo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetKind {
    Investment,
    Inventory,
    Equipment,
}

/// Linha de `assets`: capital do negocio por tipo, sem itens nem quantidades. Nao e receita
/// nem despesa e nunca entra nessas somas.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetFields {
    pub kind: AssetKind,
    pub name: String,
    /// Inteiro JSON, como `amountMinor`.
    pub value_minor: i64,
    pub acquired_on: String,
    #[serde(default)]
    pub note: Option<String>,
    /// `book` do app; ver `Book::from_field`. `Value` e nao `Book` para valor desconhecido (ou
    /// de outro tipo JSON) nao fazer a *linha* ser ignorada.
    #[serde(default)]
    pub book: Option<serde_json::Value>,
}

impl AssetFields {
    pub fn book(&self) -> Book {
        Book::from_field(self.book.as_ref())
    }
}

/// Linha de `debts`: cadastro simples com saldo devedor editavel (sem parcelas nem juros).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebtFields {
    pub name: String,
    #[serde(default)]
    pub creditor: Option<String>,
    pub principal_minor: i64,
    /// Saldo devedor atual: e o que soma em "dividas totais".
    pub balance_minor: i64,
    #[serde(default)]
    pub due_on: Option<String>,
    /// `book` do app; ver `Book::from_field`. `Value` e nao `Book` para valor desconhecido (ou
    /// de outro tipo JSON) nao fazer a *linha* ser ignorada.
    #[serde(default)]
    pub book: Option<serde_json::Value>,
}

impl DebtFields {
    pub fn book(&self) -> Book {
        Book::from_field(self.book.as_ref())
    }
}

/// Linha de `reserveMovements` ja traduzida. Nao lido: `reason` (o motivo da retirada e regra
/// do app). Nao e desserializada direto: o JSON do app nao tem `kind`, o sinal de `amountMinor`
/// decide guardar/retirar e `parse_reserve_movement` o traduz para `kind`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReserveMovementFields {
    pub reserve_id: String,
    pub kind: MovementKind,
    /// Sempre positivo aqui: o sinal vindo do JSON ja virou `kind`.
    pub amount_minor: i64,
    pub occurred_on: String,
    pub user_id: Option<String>,
    pub description: Option<String>,
    pub recurring: Option<bool>,
    pub book: Book,
}

/// Forma crua do app. Sem `deny_unknown_fields`: um `kind` legado vira campo desconhecido e e
/// ignorado, senao o sinal e o `kind` poderiam discordar.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMovementFields {
    reserve_id: String,
    amount_minor: i64,
    occurred_on: String,
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    recurring: Option<bool>,
    #[serde(default)]
    book: Option<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ContractError {
    /// Traz o motivo do serde ("missing field `reserveId`", "invalid type: ... amountMinor").
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("amountMinor nao pode ser zero")]
    AmountZero,
    #[error("amountMinor fora do intervalo")]
    AmountOutOfRange,
    #[error("occurredOn fora do formato YYYY-MM-DD")]
    OccurredOnShape,
    #[error("deadline fora do formato YYYY-MM")]
    DueMonthShape,
    #[error("recurring.since fora do formato YYYY-MM")]
    RecurringSinceShape,
    /// O app so grava meta positiva; zero ou negativo nao tem barra para desenhar.
    #[error("limitMinor deve ser positivo")]
    LimitNotPositive,
    #[error("acquiredOn fora do formato YYYY-MM-DD")]
    AcquiredOnShape,
    #[error("dueOn fora do formato YYYY-MM-DD")]
    DueOnShape,
}

pub fn parse_transaction(data: &str) -> Result<TransactionFields, ContractError> {
    let fields: TransactionFields = serde_json::from_str(data)?;
    if !is_iso_date(&fields.occurred_on) {
        return Err(ContractError::OccurredOnShape);
    }
    Ok(fields)
}

pub fn parse_category(data: &str) -> Result<CategoryFields, ContractError> {
    Ok(serde_json::from_str(data)?)
}

pub fn parse_user(data: &str) -> Result<UserFields, ContractError> {
    Ok(serde_json::from_str(data)?)
}

pub fn parse_payment_method(data: &str) -> Result<PaymentMethodFields, ContractError> {
    Ok(serde_json::from_str(data)?)
}

pub fn parse_recurrence(data: &str) -> Result<RecurrenceFields, ContractError> {
    Ok(serde_json::from_str(data)?)
}

pub fn parse_recurrence_adjustment(
    data: &str,
) -> Result<RecurrenceAdjustmentFields, ContractError> {
    Ok(serde_json::from_str(data)?)
}

pub fn parse_budget(data: &str) -> Result<BudgetFields, ContractError> {
    let fields: BudgetFields = serde_json::from_str(data)?;
    if fields.limit_minor <= 0 {
        return Err(ContractError::LimitNotPositive);
    }
    Ok(fields)
}

pub fn parse_book(data: &str) -> Result<BookFields, ContractError> {
    Ok(serde_json::from_str(data)?)
}

pub fn parse_asset(data: &str) -> Result<AssetFields, ContractError> {
    let fields: AssetFields = serde_json::from_str(data)?;
    if !is_iso_date(&fields.acquired_on) {
        return Err(ContractError::AcquiredOnShape);
    }
    Ok(fields)
}

pub fn parse_debt(data: &str) -> Result<DebtFields, ContractError> {
    let fields: DebtFields = serde_json::from_str(data)?;
    if fields.due_on.as_deref().is_some_and(|d| !is_iso_date(d)) {
        return Err(ContractError::DueOnShape);
    }
    Ok(fields)
}

pub fn parse_reserve(data: &str) -> Result<ReserveFields, ContractError> {
    let fields: ReserveFields = serde_json::from_str(data)?;
    if fields
        .due_month
        .as_deref()
        .is_some_and(|m| !is_iso_month(m))
    {
        return Err(ContractError::DueMonthShape);
    }
    // A projecao do deposito mensal compara `since` como string com o mes corrente; um formato
    // livre compararia errado sem aviso.
    if fields.recurring_since().is_some_and(|m| !is_iso_month(m)) {
        return Err(ContractError::RecurringSinceShape);
    }
    Ok(fields)
}

pub fn parse_reserve_movement(data: &str) -> Result<ReserveMovementFields, ContractError> {
    let raw: RawMovementFields = serde_json::from_str(data)?;
    if !is_iso_date(&raw.occurred_on) {
        return Err(ContractError::OccurredOnShape);
    }
    let (kind, amount_minor) = match raw.amount_minor.cmp(&0) {
        std::cmp::Ordering::Greater => (MovementKind::Deposit, raw.amount_minor),
        // `checked_abs`: `-i64::MIN` estoura (panic em debug); essa linha vira "fora do contrato".
        std::cmp::Ordering::Less => match raw.amount_minor.checked_abs() {
            Some(abs) => (MovementKind::Withdrawal, abs),
            None => return Err(ContractError::AmountOutOfRange),
        },
        // Linha com 0 e corrupta (o app nunca grava): contar em "fora do contrato", nao somar.
        std::cmp::Ordering::Equal => return Err(ContractError::AmountZero),
    };
    Ok(ReserveMovementFields {
        reserve_id: raw.reserve_id,
        kind,
        amount_minor,
        occurred_on: raw.occurred_on,
        user_id: raw.user_id,
        description: raw.description,
        recurring: raw.recurring,
        book: Book::from_field(raw.book.as_ref()),
    })
}

/// 7 bytes, `-` em 4, digitos no resto, mes 01..=12: a mesma forma do comeco de `is_iso_date`,
/// para `deadline`/`recurring.since` compararem como string com `month_of(occurredOn)` sem parsear.
pub fn is_iso_month(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 7 || b[4] != b'-' {
        return false;
    }
    if !b
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || c.is_ascii_digit())
    {
        return false;
    }
    let month = (b[5] - b'0') * 10 + (b[6] - b'0');
    (1..=12).contains(&month)
}

/// 10 bytes, `-` em 4 e 7, digitos no resto, mes 01..=12. Dia nao e validado: o hub nao julga
/// regra de negocio, e `2026-02-31` aparece cru na lista.
pub fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = b
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    if !digits {
        return false;
    }
    let month = (b[5] - b'0') * 10 + (b[6] - b'0');
    (1..=12).contains(&month)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn tx() -> Value {
        json!({
            "kind": "expense",
            "description": "Mercado",
            "amountMinor": 12345,
            "currency": "BRL",
            "occurredOn": "2026-09-14",
            "categoryId": "01HZZZZZZZZZZZZZZZZZZZZZC1",
            "paymentMethodId": "01HZZZZZZZZZZZZZZZZZZZZZM1",
            "userId": "01HZZZZZZZZZZZZZZZZZZZZZP1",
            "recurrenceId": null,
            "cashbackMinor": 10,
            "foo": 1
        })
    }

    fn with(mut v: Value, key: &str, value: Value) -> String {
        v[key] = value;
        v.to_string()
    }

    fn without(mut v: Value, key: &str) -> String {
        v.as_object_mut().unwrap().remove(key);
        v.to_string()
    }

    fn err_text<T: std::fmt::Debug>(r: Result<T, ContractError>) -> String {
        r.expect_err("devia falhar").to_string()
    }

    #[test]
    fn transacao_valida_com_campos_extras() {
        let t = parse_transaction(&tx().to_string()).unwrap();
        assert_eq!(t.kind, Kind::Expense);
        assert_eq!(t.amount_minor, 12345);
        assert_eq!(t.occurred_on, "2026-09-14");
        assert_eq!(t.user_id.as_deref(), Some("01HZZZZZZZZZZZZZZZZZZZZZP1"));
        assert_eq!(t.recurrence_id, None);
    }

    #[test]
    fn linha_antiga_sem_report_state_conta_normalmente() {
        let t = parse_transaction(&tx().to_string()).unwrap();
        assert_eq!(t.report_state(), None);
        let t = parse_transaction(&with(tx(), "reportState", Value::Null)).unwrap();
        assert_eq!(t.report_state(), None);
    }

    #[test]
    fn report_state_conhecido_e_lido() {
        let t = parse_transaction(&with(tx(), "reportState", json!("transfer"))).unwrap();
        assert_eq!(t.report_state(), Some(ReportState::Transfer));
        let t = parse_transaction(&with(tx(), "reportState", json!("hidden"))).unwrap();
        assert_eq!(t.report_state(), Some(ReportState::Hidden));
    }

    #[test]
    fn report_state_desconhecido_ou_de_outro_tipo_nao_derruba_a_linha() {
        for future in [
            json!("archived"),
            json!(3),
            json!({ "x": 1 }),
            json!(["hidden"]),
        ] {
            let t = parse_transaction(&with(tx(), "reportState", future)).unwrap();
            assert_eq!(t.report_state(), None);
        }
    }

    #[test]
    fn kind_desconhecido_e_erro() {
        let e = err_text(parse_transaction(&with(tx(), "kind", json!("transfer"))));
        assert!(e.contains("transfer") || e.contains("variant"), "{e}");
        let e = err_text(parse_transaction(&without(tx(), "kind")));
        assert!(e.contains("kind"), "{e}");
    }

    #[test]
    fn amount_minor_precisa_ser_inteiro() {
        for bad in [json!(12.5), json!("12")] {
            let e = err_text(parse_transaction(&with(tx(), "amountMinor", bad)));
            assert!(
                e.contains("amountMinor") || e.contains("invalid type"),
                "{e}"
            );
        }
        let e = err_text(parse_transaction(&without(tx(), "amountMinor")));
        assert!(e.contains("amountMinor"), "{e}");
    }

    #[test]
    fn occurred_on_fora_do_formato() {
        for bad in [
            json!("2026-9-1"),
            json!("2026-13-01"),
            json!("2026-00-01"),
            json!("2026-09-01T00:00"),
        ] {
            let e = err_text(parse_transaction(&with(tx(), "occurredOn", bad)));
            assert!(e.contains("occurredOn"), "{e}");
        }
        assert!(parse_transaction(&with(tx(), "occurredOn", json!(20260901))).is_err());
        // Dia impossivel passa: o hub nao julga.
        assert!(parse_transaction(&with(tx(), "occurredOn", json!("2026-02-31"))).is_ok());
    }

    #[test]
    fn anulaveis_ausentes_ou_nulos_viram_none() {
        let mut v = tx();
        for key in ["categoryId", "paymentMethodId", "userId", "recurrenceId"] {
            v.as_object_mut().unwrap().remove(key);
        }
        let t = parse_transaction(&v.to_string()).unwrap();
        assert_eq!(t.category_id, None);
        assert_eq!(t.payment_method_id, None);
        assert_eq!(t.user_id, None);
        assert_eq!(t.recurrence_id, None);
        let t = parse_transaction(&with(tx(), "userId", Value::Null)).unwrap();
        assert_eq!(t.user_id, None);
    }

    #[test]
    fn descricao_com_tipo_errado() {
        assert!(parse_transaction(&with(tx(), "description", json!(5))).is_err());
    }

    #[test]
    fn categorias() {
        let ok =
            json!({ "name": "Mercado", "color": "orange", "icon": "utensils", "kind": "expense" });
        assert!(parse_category(&ok.to_string()).is_ok());
        let neon = with(ok.clone(), "color", json!("neon"));
        assert_eq!(parse_category(&neon).unwrap().color, "neon");
        assert!(err_text(parse_category(&without(ok.clone(), "color"))).contains("color"));
        assert!(err_text(parse_category(&without(ok.clone(), "icon"))).contains("icon"));
        assert_eq!(parse_category(&ok.to_string()).unwrap().merged_into, None);
        let nulo = with(ok.clone(), "mergedInto", Value::Null);
        assert_eq!(parse_category(&nulo).unwrap().merged_into, None);
        let fundida = with(ok.clone(), "mergedInto", json!("C1"));
        assert_eq!(
            parse_category(&fundida).unwrap().merged_into.as_deref(),
            Some("C1")
        );
        // O serde_json nao cita o campo em tipo errado de valor ("invalid type: integer `5`,
        // expected a string"); a linha cai em `ignored` do mesmo jeito.
        let e = err_text(parse_category(&with(ok, "mergedInto", json!(5))));
        assert!(e.contains("invalid type"), "{e}");
    }

    #[test]
    fn usuarios() {
        let ok = json!({ "name": "Ana", "color": "fuchsia" });
        assert_eq!(parse_user(&ok.to_string()).unwrap().avatar, None);
        let big = format!("data:image/png;base64,{}", "A".repeat(60 * 1024));
        let u = parse_user(&with(ok.clone(), "avatar", json!(big))).unwrap();
        assert_eq!(u.avatar.map(|a| a.len()), Some(big.len()));
        assert!(parse_user(&with(ok, "avatar", json!(7))).is_err());
    }

    #[test]
    fn formas_de_pagamento() {
        let ok = json!({ "name": "Pix", "icon": "zap", "color": "teal", "kind": "pix" });
        assert!(parse_payment_method(&ok.to_string()).is_ok());
        assert!(err_text(parse_payment_method(&without(ok.clone(), "name"))).contains("name"));
        assert!(err_text(parse_payment_method(&without(ok.clone(), "icon"))).contains("icon"));
        assert_eq!(
            parse_payment_method(&ok.to_string()).unwrap().merged_into,
            None
        );
        let nulo = with(ok.clone(), "mergedInto", Value::Null);
        assert_eq!(parse_payment_method(&nulo).unwrap().merged_into, None);
        let fundida = with(ok.clone(), "mergedInto", json!("M1"));
        assert_eq!(
            parse_payment_method(&fundida)
                .unwrap()
                .merged_into
                .as_deref(),
            Some("M1")
        );
        let e = err_text(parse_payment_method(&with(ok, "mergedInto", json!(5))));
        assert!(e.contains("invalid type"), "{e}");
    }

    #[test]
    fn recorrencias() {
        let ok = json!({ "frequency": "monthly", "description": "Aluguel" });
        assert_eq!(
            parse_recurrence(&ok.to_string()).unwrap().frequency,
            "monthly"
        );
        assert!(err_text(parse_recurrence(&without(ok, "frequency"))).contains("frequency"));
    }

    // ---- contrato ampliado (spec Grafite 4.1-4.4) ----

    /// Serie como o app grava hoje (`recurrence.ts`), com uma regra completa.
    fn series() -> Value {
        json!({
            "id": "RCON",
            "createdAt": "2026-11-01T12:00:00.000Z",
            "updatedAt": "1793000000000-0000-01JZ0000000000000000000DV1",
            "deletedAt": null,
            "kind": "expense",
            "description": "Contas da casa",
            "amountMinor": 53901,
            "currency": "BRL",
            "categoryId": "CAT-CONTAS",
            "paymentMethodId": null,
            "cashbackMinor": null,
            "frequency": "monthly",
            "scheduleType": "dayOfMonth",
            "scheduleN": 10,
            "startOn": "2026-11-10",
            "endOn": null,
            "active": true,
            "variable": null,
            "firstAmountMinor": null,
            "userId": "UL"
        })
    }

    #[test]
    fn regra_completa_e_interpretada() {
        let r = parse_recurrence(&series().to_string()).unwrap();
        assert_eq!(r.kind(), Kind::Expense);
        assert_eq!(r.description(), Some("Contas da casa"));
        assert_eq!(r.category_id(), Some("CAT-CONTAS"));
        assert_eq!(r.payment_method_id(), None);
        assert_eq!(r.user_id(), Some("UL"));
        assert!(!r.is_variable());
        assert_eq!(
            r.rule(),
            Some(RecurrenceRule {
                frequency: Frequency::Monthly,
                schedule: Some(Schedule::DayOfMonth(10)),
                kind: Kind::Expense,
                amount_minor: 53901,
                start_on: "2026-11-10".into(),
                end_on: None,
                active: true,
                variable: false,
                first_amount_minor: None,
            })
        );
        let r = parse_recurrence(&with(
            with(series().clone(), "scheduleType", json!("nthBusinessDay"))
                .parse()
                .unwrap(),
            "scheduleN",
            json!(5),
        ))
        .unwrap();
        let rule = r.rule().unwrap();
        assert_eq!(rule.schedule, Some(Schedule::NthBusinessDay(5)));
        let r = parse_recurrence(&with(series(), "endOn", json!("2027-01-10"))).unwrap();
        assert_eq!(r.rule().unwrap().end_on.as_deref(), Some("2027-01-10"));
    }

    #[test]
    fn recorrencia_so_com_frequency_e_valida_e_sem_regra() {
        let r = parse_recurrence(r#"{"frequency":"monthly"}"#).unwrap();
        assert_eq!(r.rule(), None);
        assert_eq!(r.kind(), Kind::Expense);
        assert!(!r.is_variable());
        assert_eq!(r.category_id(), None);
    }

    #[test]
    fn campo_novo_com_tipo_errado_nao_derruba_a_linha() {
        // Todos juntos (o caso da spec 4.6) e cada um sozinho sobre uma regra completa.
        let mut all = series();
        for (key, value) in [
            ("amountMinor", json!("12")),
            ("active", json!("sim")),
            ("scheduleN", json!(1.5)),
            ("variable", json!(3)),
        ] {
            all[key] = value.clone();
            let one = parse_recurrence(&with(series(), key, value.clone())).unwrap();
            if key == "variable" {
                // `variable` nao invalida a regra: so `true` liga "valor varia".
                assert!(!one.rule().unwrap().variable, "{key}");
                assert!(!one.is_variable());
            } else {
                assert_eq!(one.rule(), None, "{key} = {value}");
            }
        }
        let r = parse_recurrence(&all.to_string()).unwrap();
        assert_eq!(r.rule(), None);
        for (key, value) in [
            ("kind", json!(7)),
            ("description", json!(["x"])),
            ("categoryId", json!(5)),
            ("paymentMethodId", json!({})),
            ("scheduleType", json!("lastFriday")),
            ("scheduleN", json!(0)),
            ("scheduleN", json!(-2)),
            ("startOn", json!("")),
            ("startOn", json!("10/11/2026")),
            ("startOn", json!(20261110)),
            ("endOn", json!("logo")),
            ("frequency", json!("daily")),
            ("firstAmountMinor", json!("x")),
            ("userId", json!(1)),
        ] {
            let r = parse_recurrence(&with(series(), key, value.clone()));
            assert!(r.is_ok(), "{key} = {value} derrubou a linha");
        }
        // Os que invalidam a regra.
        for (key, value) in [
            ("scheduleType", json!("lastFriday")),
            ("scheduleN", json!(0)),
            ("startOn", json!("")),
            ("startOn", json!("10/11/2026")),
            ("endOn", json!("logo")),
            ("frequency", json!("daily")),
        ] {
            let r = parse_recurrence(&with(series(), key, value.clone())).unwrap();
            assert_eq!(r.rule(), None, "{key} = {value}");
        }
        // Os que so degradam o proprio campo.
        let r = parse_recurrence(&with(series(), "categoryId", json!(5))).unwrap();
        assert_eq!(r.category_id(), None);
        assert!(r.rule().is_some());
        let r = parse_recurrence(&with(series(), "firstAmountMinor", json!("x"))).unwrap();
        assert_eq!(r.rule().unwrap().first_amount_minor, None);
    }

    #[test]
    fn frequency_continua_obrigatoria_e_texto() {
        assert!(parse_recurrence(&without(series(), "frequency")).is_err());
        assert!(parse_recurrence(&with(series(), "frequency", json!(1))).is_err());
    }

    #[test]
    fn kind_que_nao_e_income_e_despesa() {
        for value in [json!("transfer"), json!(3), Value::Null] {
            let r = parse_recurrence(&with(series(), "kind", value)).unwrap();
            assert_eq!(r.kind(), Kind::Expense);
            assert_eq!(r.rule().unwrap().kind, Kind::Expense);
        }
        let r = parse_recurrence(&with(series(), "kind", json!("income"))).unwrap();
        assert_eq!(r.rule().unwrap().kind, Kind::Income);
    }

    #[test]
    fn active_ausente_ou_nulo_vale_true_e_false_pausa() {
        // [inventada, spec 4.2] ausencia e app antigo, que so gravava serie ativa. `null` chega
        // ao hub igual a ausente (`Option<Value>`), e o app nunca grava `null` em `active`.
        let r = parse_recurrence(&without(series(), "active")).unwrap();
        assert!(r.rule().unwrap().active);
        let r = parse_recurrence(&with(series(), "active", Value::Null)).unwrap();
        assert!(r.rule().unwrap().active);
        let r = parse_recurrence(&with(series(), "active", json!(false))).unwrap();
        assert!(!r.rule().unwrap().active);
    }

    #[test]
    fn semanal_e_quinzenal_nao_exigem_agenda_do_mes() {
        for frequency in ["weekly", "biweekly"] {
            let mut v = with(series(), "frequency", json!(frequency))
                .parse::<Value>()
                .unwrap();
            v.as_object_mut().unwrap().remove("scheduleType");
            v["scheduleN"] = json!("x");
            let rule = parse_recurrence(&v.to_string()).unwrap().rule().unwrap();
            assert_eq!(rule.schedule, None, "{frequency}");
            assert!(rule.frequency.is_day_step());
        }
        // Mensais exigem.
        assert_eq!(
            parse_recurrence(&without(series(), "scheduleType"))
                .unwrap()
                .rule(),
            None
        );
        assert_eq!(
            parse_recurrence(&without(series(), "scheduleN"))
                .unwrap()
                .rule(),
            None
        );
    }

    #[test]
    fn todas_as_frequencias_do_app_sao_conhecidas() {
        for (text, f) in [
            ("weekly", Frequency::Weekly),
            ("biweekly", Frequency::Biweekly),
            ("monthly", Frequency::Monthly),
            ("bimonthly", Frequency::Bimonthly),
            ("quarterly", Frequency::Quarterly),
            ("semiannual", Frequency::Semiannual),
            ("annual", Frequency::Annual),
        ] {
            assert_eq!(Frequency::parse(text), Some(f));
        }
        assert_eq!(Frequency::parse("daily"), None);
    }

    #[test]
    fn valor_varia_e_primeira_parcela() {
        let r = parse_recurrence(&with(series(), "variable", json!(true))).unwrap();
        assert!(r.is_variable());
        assert!(r.rule().unwrap().variable);
        let r = parse_recurrence(&with(series(), "firstAmountMinor", json!(3334))).unwrap();
        assert_eq!(r.rule().unwrap().first_amount_minor, Some(3334));
        for bad in [json!(0), json!(-5), json!(12.5)] {
            let r = parse_recurrence(&with(series(), "firstAmountMinor", bad)).unwrap();
            assert_eq!(r.rule().unwrap().first_amount_minor, None);
        }
    }

    #[test]
    fn transacao_sem_estimated_nem_occurrence_key_e_confirmada() {
        let t = parse_transaction(&tx().to_string()).unwrap();
        assert!(!t.estimated());
        assert_eq!(t.occurrence_key(), None);
        for value in [Value::Null, json!(false), json!("true"), json!(1)] {
            let t = parse_transaction(&with(tx(), "estimated", value)).unwrap();
            assert!(!t.estimated());
        }
        let t = parse_transaction(&with(tx(), "estimated", json!(true))).unwrap();
        assert!(t.estimated());
        let t = parse_transaction(&with(tx(), "occurrenceKey", json!("R1:2026-11"))).unwrap();
        assert_eq!(t.occurrence_key(), Some("R1:2026-11"));
        for value in [Value::Null, json!(5), json!(["R1:2026-11"])] {
            let t = parse_transaction(&with(tx(), "occurrenceKey", value)).unwrap();
            assert_eq!(t.occurrence_key(), None);
        }
    }

    fn card() -> Value {
        json!({
            "name": "Mercado Pago", "icon": "credit-card", "color": "violet",
            "kind": "credit", "bank": "Mercado Pago", "closingDay": 28, "dueDay": 8
        })
    }

    #[test]
    fn cartao_completo() {
        let p = parse_payment_method(&card().to_string()).unwrap();
        assert!(p.is_credit());
        assert!(p.is_complete_card());
        assert_eq!(
            p.card(),
            Some(Card {
                bank: "Mercado Pago".into(),
                closing_day: 28,
                due_day: 8
            })
        );
    }

    #[test]
    fn forma_de_pagamento_antiga_sem_cartao_e_valida() {
        let p = parse_payment_method(r#"{"name":"Pix","icon":"zap"}"#).unwrap();
        assert!(!p.is_credit());
        assert!(!p.is_complete_card());
        // Credito sem os dados do cartao (o "Cartao de credito" generico): valido, sem fatura.
        for key in ["bank", "closingDay", "dueDay"] {
            let p = parse_payment_method(&without(card(), key)).unwrap();
            assert!(p.is_credit());
            assert!(!p.is_complete_card(), "{key}");
        }
        for (key, value) in [
            ("bank", json!("   ")),
            ("bank", json!(5)),
            ("kind", json!("debit")),
            ("kind", json!(1)),
            ("closingDay", json!("28")),
            ("dueDay", json!(8.5)),
        ] {
            let p = parse_payment_method(&with(card(), key, value.clone())).unwrap();
            assert!(!p.is_complete_card(), "{key} = {value}");
        }
        // Banco com espaco sobrando vale, aparado.
        let p = parse_payment_method(&with(card(), "bank", json!(" Nubank "))).unwrap();
        assert_eq!(p.card().map(|c| c.bank), Some("Nubank".to_string()));
    }

    #[test]
    fn dia_31_e_32_de_cartao() {
        let p = parse_payment_method(&with(card(), "closingDay", json!(31))).unwrap();
        assert_eq!(p.card().map(|c| c.closing_day), Some(31));
        let p = parse_payment_method(&with(card(), "dueDay", json!(31))).unwrap();
        assert_eq!(p.card().map(|c| c.due_day), Some(31));
        for key in ["closingDay", "dueDay"] {
            for bad in [json!(32), json!(0), json!(-1)] {
                let p = parse_payment_method(&with(card(), key, bad.clone())).unwrap();
                assert!(!p.is_complete_card(), "{key} = {bad}");
            }
        }
    }

    #[test]
    fn reajuste_valido_e_fora_do_contrato() {
        let ok = json!({
            "id": "A1", "recurrenceId": "R1", "fromPeriod": "2026-12",
            "amountMinor": 60000, "book": "business", "deletedAt": null
        });
        let a = parse_recurrence_adjustment(&ok.to_string()).unwrap();
        assert_eq!(a.recurrence_id, "R1");
        assert_eq!(a.from_period, "2026-12");
        assert_eq!(a.amount_minor, 60_000);
        assert_eq!(a.book(), Book::Business);
        // Semanal: a competencia e a data.
        let weekly = with(ok.clone(), "fromPeriod", json!("2026-12-07"));
        assert_eq!(
            parse_recurrence_adjustment(&weekly).unwrap().from_period,
            "2026-12-07"
        );
        for key in ["recurrenceId", "fromPeriod", "amountMinor"] {
            assert!(
                parse_recurrence_adjustment(&without(ok.clone(), key)).is_err(),
                "{key}"
            );
        }
        for (key, value) in [
            ("amountMinor", json!("600")),
            ("amountMinor", json!(1.5)),
            ("fromPeriod", json!(202612)),
            ("recurrenceId", Value::Null),
        ] {
            assert!(
                parse_recurrence_adjustment(&with(ok.clone(), key, value)).is_err(),
                "{key}"
            );
        }
    }

    #[test]
    fn json_que_nao_e_objeto() {
        for bad in ["[]", "null", "{", "\"texto\"", "12"] {
            assert!(parse_transaction(bad).is_err(), "{bad}");
            assert!(parse_category(bad).is_err(), "{bad}");
            assert!(parse_user(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn data_iso() {
        assert!(is_iso_date("2026-09-24"));
        assert!(is_iso_date("2026-12-31"));
        assert!(!is_iso_date("2026-9-24"));
        assert!(!is_iso_date("2026/09/24"));
        assert!(!is_iso_date("2026-09-2a"));
        assert!(!is_iso_date("2026-13-01"));
        assert!(!is_iso_date(""));
    }

    fn reserve() -> Value {
        json!({
            "id": "01JZ0000000000000000000RE1",
            "createdAt": "2026-10-01T18:00:00.000Z",
            "updatedAt": "0001790000000-0000-01JZ0000000000000000000DV1",
            "deletedAt": null,
            "kind": "goal",
            "name": "Viagem",
            "icon": "gift",
            "color": "rose",
            "targetMinor": 500000,
            "multiple": null,
            "essentialCategoryIds": null,
            "essentialOverrideMinor": null,
            "deadline": "2027-07",
            "recurring": { "amountMinor": 30000, "day": 10, "since": "2026-11" },
            "foo": 1
        })
    }

    fn movement() -> Value {
        json!({
            "id": "01JZ0000000000000000000MV1",
            "createdAt": "2026-09-06T12:00:00.000Z",
            "updatedAt": "0001790000000-0000-01JZ0000000000000000000DV1",
            "deletedAt": null,
            "reserveId": "RE1",
            "amountMinor": 50000,
            "occurredOn": "2026-09-06",
            "userId": "U2",
            "description": "Guardado todo mês",
            "reason": null,
            "recurring": true
        })
    }

    #[test]
    fn reserva_valida_com_campos_extras() {
        let r = parse_reserve(&reserve().to_string()).unwrap();
        assert_eq!(r.kind, ReserveKind::Goal);
        assert_eq!(r.goal_minor, Some(500_000));
        assert_eq!(r.due_month.as_deref(), Some("2027-07"));
        assert_eq!(r.recurring_amount_minor(), Some(30_000));
        assert_eq!(r.recurring_since(), Some("2026-11"));
        assert_eq!(r.essential_override_minor, None);
        assert_eq!(r.multiple, None);
        assert_eq!(r.essential_category_ids, None);
    }

    #[test]
    fn reserva_kind_desconhecido_ou_ausente() {
        let e = err_text(parse_reserve(&with(reserve(), "kind", json!("box"))));
        assert!(e.contains("box") || e.contains("variant"), "{e}");
        // "pot" foi o nome antigo: sem alias, cai como qualquer valor desconhecido.
        assert!(parse_reserve(&with(reserve(), "kind", json!("pot"))).is_err());
        let e = err_text(parse_reserve(&without(reserve(), "kind")));
        assert!(e.contains("kind"), "{e}");
    }

    #[test]
    fn reserva_multiple_e_u8() {
        let r = parse_reserve(&with(reserve(), "multiple", json!(6))).unwrap();
        assert_eq!(r.multiple, Some(6));
        for bad in [json!(6.5), json!("6"), json!(300), json!(-1)] {
            assert!(
                parse_reserve(&with(reserve(), "multiple", bad.clone())).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn reserva_categorias_essenciais() {
        let r = parse_reserve(&with(
            reserve(),
            "essentialCategoryIds",
            json!(["C1", "C2"]),
        ))
        .unwrap();
        assert_eq!(
            r.essential_category_ids,
            Some(vec!["C1".to_string(), "C2".to_string()])
        );
        for bad in [json!("C1"), json!([1])] {
            assert!(
                parse_reserve(&with(reserve(), "essentialCategoryIds", bad.clone())).is_err(),
                "{bad}"
            );
        }
        let r = parse_reserve(&without(reserve(), "essentialCategoryIds")).unwrap();
        assert_eq!(r.essential_category_ids, None);
    }

    #[test]
    fn reserva_due_month_fora_do_formato() {
        for bad in [json!("2027-7"), json!("2027-13"), json!("2027-07-01")] {
            let e = err_text(parse_reserve(&with(reserve(), "deadline", bad)));
            assert!(e.contains("deadline"), "{e}");
        }
        // Numero e tipo errado do serde, que tambem cita o campo.
        let e = err_text(parse_reserve(&with(reserve(), "deadline", json!(202707))));
        assert!(e.contains("invalid type"), "{e}");
        let r = parse_reserve(&with(reserve(), "deadline", Value::Null)).unwrap();
        assert_eq!(r.due_month, None);
        let r = parse_reserve(&without(reserve(), "deadline")).unwrap();
        assert_eq!(r.due_month, None);
    }

    #[test]
    fn reserva_goal_fracionado_e_anulaveis_ausentes() {
        assert!(parse_reserve(&with(reserve(), "targetMinor", json!(12.5))).is_err());
        let r = parse_reserve(
            &json!({ "kind": "goal", "name": "X", "icon": "tag", "color": "slate" }).to_string(),
        )
        .unwrap();
        assert_eq!(r.goal_minor, None);
        assert_eq!(r.multiple, None);
        assert_eq!(r.essential_category_ids, None);
        assert_eq!(r.due_month, None);
        assert_eq!(r.recurring, None);
        assert_eq!(r.recurring_amount_minor(), None);
        assert_eq!(r.essential_override_minor, None);
    }

    #[test]
    fn reserva_recurring_tipos_e_formato() {
        let r = parse_reserve(&with(reserve(), "recurring", Value::Null)).unwrap();
        assert_eq!(r.recurring_amount_minor(), None);
        assert!(parse_reserve(&with(reserve(), "recurring", json!("sim"))).is_err());
        assert!(
            parse_reserve(&with(
                reserve(),
                "recurring",
                json!({ "amountMinor": 12.5 })
            ))
            .is_err()
        );
        assert!(parse_reserve(&with(reserve(), "recurring", json!({ "day": 10 }))).is_err());
        let e = err_text(parse_reserve(&with(
            reserve(),
            "recurring",
            json!({ "amountMinor": 100, "since": "2026-1" }),
        )));
        assert!(e.contains("recurring.since"), "{e}");
        let r =
            parse_reserve(&with(reserve(), "recurring", json!({ "amountMinor": 100 }))).unwrap();
        assert_eq!(r.recurring_since(), None);
    }

    #[test]
    fn reserva_essential_override_lido() {
        let r = parse_reserve(&with(reserve(), "essentialOverrideMinor", json!(250000))).unwrap();
        assert_eq!(r.essential_override_minor, Some(250_000));
        assert!(parse_reserve(&with(reserve(), "essentialOverrideMinor", json!("x"))).is_err());
    }

    #[test]
    fn reserva_de_emergencia() {
        let ok = json!({
            "kind": "emergency",
            "name": "Reserva",
            "icon": "x",
            "color": "slate",
            "multiple": 6,
            "essentialCategoryIds": ["C1"]
        });
        let r = parse_reserve(&ok.to_string()).unwrap();
        assert_eq!(r.kind, ReserveKind::Emergency);
        assert_eq!(r.multiple, Some(6));
    }

    #[test]
    fn reserva_campos_do_app_chegam_projetados() {
        let r = parse_reserve(&reserve().to_string()).unwrap();
        assert_eq!(r.goal_minor, Some(500_000));
        assert_eq!(r.due_month.as_deref(), Some("2027-07"));
        assert_eq!(r.recurring_amount_minor(), Some(30_000));
        assert_eq!(r.recurring.as_ref().and_then(|x| x.day), Some(10));
        assert_eq!(r.recurring_since(), Some("2026-11"));
    }

    #[test]
    fn reserva_recurring_nulo_ou_ausente_passa() {
        let r = parse_reserve(&with(reserve(), "recurring", Value::Null)).unwrap();
        assert_eq!(r.recurring, None);
        let r = parse_reserve(&without(reserve(), "recurring")).unwrap();
        assert_eq!(r.recurring, None);
    }

    #[test]
    fn reserva_essential_override_nulo_ausente_e_tipos_errados() {
        let r = parse_reserve(&with(reserve(), "essentialOverrideMinor", json!(65000))).unwrap();
        assert_eq!(r.essential_override_minor, Some(65_000));
        let r = parse_reserve(&with(reserve(), "essentialOverrideMinor", Value::Null)).unwrap();
        assert_eq!(r.essential_override_minor, None);
        let r = parse_reserve(&without(reserve(), "essentialOverrideMinor")).unwrap();
        assert_eq!(r.essential_override_minor, None);
        for bad in [json!(12.5), json!("65000")] {
            assert!(
                parse_reserve(&with(reserve(), "essentialOverrideMinor", bad.clone())).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn reserva_de_emergencia_como_o_app_grava() {
        let row = json!({
            "id": "01JZ0000000000000000000RE2",
            "createdAt": "2026-10-01T18:00:00.000Z",
            "updatedAt": "1759341600000-0000-01JZ0000000000000000000DV1",
            "deletedAt": null,
            "kind": "emergency",
            "name": "Reserva de emergência",
            "icon": "lifebuoy",
            "color": "violet",
            "targetMinor": null,
            "multiple": 6,
            "essentialCategoryIds": ["C1", "C2"],
            "essentialOverrideMinor": 350000,
            "deadline": null,
            "recurring": { "amountMinor": 50000, "day": 5, "since": "2026-11" }
        });
        let r = parse_reserve(&row.to_string()).unwrap();
        assert_eq!(r.kind, ReserveKind::Emergency);
        assert_eq!(r.multiple, Some(6));
        assert_eq!(r.essential_override_minor, Some(350_000));
        assert_eq!(r.goal_minor, None);
        assert_eq!(r.due_month, None);
        assert_eq!(r.recurring_amount_minor(), Some(50_000));
    }

    #[test]
    fn movimentacao_valida() {
        let m = parse_reserve_movement(&movement().to_string()).unwrap();
        assert_eq!(m.reserve_id, "RE1");
        assert_eq!(m.kind, MovementKind::Deposit);
        assert_eq!(m.amount_minor, 50_000);
        assert_eq!(m.recurring, Some(true));
    }

    #[test]
    fn movimentacao_positiva_e_deposito() {
        let m = parse_reserve_movement(&with(movement(), "amountMinor", json!(50_000))).unwrap();
        assert_eq!(m.kind, MovementKind::Deposit);
        assert_eq!(m.amount_minor, 50_000);
    }

    #[test]
    fn movimentacao_negativa_e_retirada() {
        let mut v = movement();
        v["amountMinor"] = json!(-15_000);
        v["reason"] = json!("health");
        let m = parse_reserve_movement(&v.to_string()).unwrap();
        assert_eq!(m.kind, MovementKind::Withdrawal);
        assert_eq!(m.amount_minor, 15_000);
    }

    #[test]
    fn movimentacao_zero_e_erro() {
        let e = err_text(parse_reserve_movement(&with(
            movement(),
            "amountMinor",
            json!(0),
        )));
        assert!(e.contains("zero"), "{e}");
    }

    #[test]
    fn movimentacao_i64_min_e_erro_sem_panic() {
        let e = err_text(parse_reserve_movement(&with(
            movement(),
            "amountMinor",
            json!(i64::MIN),
        )));
        assert!(e.contains("intervalo"), "{e}");
        let m =
            parse_reserve_movement(&with(movement(), "amountMinor", json!(i64::MIN + 1))).unwrap();
        assert_eq!(m.kind, MovementKind::Withdrawal);
        assert_eq!(m.amount_minor, i64::MAX);
    }

    #[test]
    fn movimentacao_kind_extra_e_ignorado() {
        for kind in ["withdrawal", "transfer"] {
            let m = parse_reserve_movement(&with(movement(), "kind", json!(kind))).unwrap();
            assert_eq!(m.kind, MovementKind::Deposit);
            assert_eq!(m.amount_minor, 50_000);
        }
    }

    #[test]
    fn movimentacao_valor_invalido() {
        for bad in [json!(12.5), json!("12")] {
            assert!(parse_reserve_movement(&with(movement(), "amountMinor", bad)).is_err());
        }
        assert!(parse_reserve_movement(&without(movement(), "amountMinor")).is_err());
    }

    #[test]
    fn movimentacao_data_e_campos() {
        for bad in [
            json!("2026-9-6"),
            json!("2026-13-01"),
            json!("2026-09-06T00:00"),
        ] {
            let e = err_text(parse_reserve_movement(&with(movement(), "occurredOn", bad)));
            assert!(e.contains("occurredOn"), "{e}");
        }
        assert!(
            parse_reserve_movement(&with(movement(), "occurredOn", json!("2026-02-31"))).is_ok()
        );
        assert!(parse_reserve_movement(&without(movement(), "reserveId")).is_err());
        assert!(parse_reserve_movement(&with(movement(), "recurring", json!("sim"))).is_err());
        let mut v = movement();
        for key in ["userId", "description", "recurring"] {
            v.as_object_mut().unwrap().remove(key);
        }
        let m = parse_reserve_movement(&v.to_string()).unwrap();
        assert_eq!(m.user_id, None);
        assert_eq!(m.description, None);
        assert_eq!(m.recurring, None);
    }

    #[test]
    fn mes_iso() {
        assert!(is_iso_month("2027-07"));
        assert!(is_iso_month("2027-12"));
        for bad in ["2027-7", "2027-13", "2027-00", "2027/07", "2027-07-01", ""] {
            assert!(!is_iso_month(bad), "{bad}");
        }
    }

    #[test]
    fn reservas_json_que_nao_e_objeto() {
        for bad in ["[]", "null", "{"] {
            assert!(parse_reserve(bad).is_err(), "{bad}");
            assert!(parse_reserve_movement(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn meta_valida_le_categoria_limite_e_versao() {
        let b = parse_budget(
            r#"{"categoryId":"C1","limitMinor":50000,"updatedAt":"1754568000000-0000-AAA","dirty":0}"#,
        )
        .unwrap();
        assert_eq!(b.category_id, "C1");
        assert_eq!(b.limit_minor, 50_000);
        assert_eq!(b.updated_at.as_deref(), Some("1754568000000-0000-AAA"));
    }

    #[test]
    fn meta_sem_versao_e_valida() {
        let b = parse_budget(r#"{"categoryId":"C1","limitMinor":1}"#).unwrap();
        assert_eq!(b.updated_at, None);
    }

    #[test]
    fn meta_fora_do_contrato_e_erro() {
        for bad in [
            r#"{"limitMinor":100}"#,
            r#"{"categoryId":"C1"}"#,
            r#"{"categoryId":"C1","limitMinor":"x"}"#,
            r#"{"categoryId":"C1","limitMinor":0}"#,
            r#"{"categoryId":"C1","limitMinor":-5}"#,
        ] {
            assert!(parse_budget(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn book_ausente_nulo_e_desconhecido_valem_pessoal() {
        let t = parse_transaction(&tx().to_string()).unwrap();
        assert_eq!(t.book(), Book::Personal);
        for value in [Value::Null, json!("personal"), json!("futuro"), json!(7)] {
            let t = parse_transaction(&with(tx(), "book", value)).unwrap();
            assert_eq!(t.book(), Book::Personal);
        }
        let t = parse_transaction(&with(tx(), "book", json!("business"))).unwrap();
        assert_eq!(t.book(), Book::Business);
    }

    #[test]
    fn book_e_lido_em_todas_as_tabelas_com_o_campo() {
        let biz = |mut v: Value| {
            v["book"] = json!("business");
            v.to_string()
        };
        assert_eq!(
            parse_category(&biz(json!({"name":"Vendas","color":"green","icon":"x"})))
                .unwrap()
                .book(),
            Book::Business
        );
        assert_eq!(
            parse_payment_method(&biz(json!({"name":"Pix","icon":"x"})))
                .unwrap()
                .book(),
            Book::Business
        );
        assert_eq!(
            parse_recurrence(&biz(json!({"frequency":"monthly"})))
                .unwrap()
                .book(),
            Book::Business
        );
        assert_eq!(
            parse_budget(&biz(json!({"categoryId":"C1","limitMinor":5})))
                .unwrap()
                .book(),
            Book::Business
        );
        let reserve = json!({"kind":"goal","name":"R","icon":"i","color":"c"});
        assert_eq!(
            parse_reserve(&biz(reserve.clone())).unwrap().book(),
            Book::Business
        );
        assert_eq!(
            parse_reserve(&reserve.to_string()).unwrap().book(),
            Book::Personal
        );
        let movement = json!({"reserveId":"R1","amountMinor":100,"occurredOn":"2026-09-01"});
        assert_eq!(
            parse_reserve_movement(&biz(movement.clone())).unwrap().book,
            Book::Business
        );
        assert_eq!(
            parse_reserve_movement(&movement.to_string()).unwrap().book,
            Book::Personal
        );
        let unknown = with(movement, "book", json!("outra"));
        assert_eq!(
            parse_reserve_movement(&unknown).unwrap().book,
            Book::Personal
        );
    }

    #[test]
    fn conta_comercial_le_nome_cor_e_icone() {
        let b = parse_book(
            r#"{"kind":"business","name":"Padaria","color":"amber","icon":"storefront","x":1}"#,
        )
        .unwrap();
        assert_eq!(b.kind(), Book::Business);
        assert_eq!(b.name, "Padaria");
        let b = parse_book(r#"{"kind":"futuro","name":"N","color":"c","icon":"i"}"#).unwrap();
        assert_eq!(b.kind(), Book::Personal);
        assert!(parse_book(r#"{"name":"N","color":"c","icon":"i"}"#).is_err());
    }

    #[test]
    fn bem_valido_e_fora_do_contrato() {
        let asset = json!({
            "kind": "inventory", "name": "Farinha", "valueMinor": 150000,
            "acquiredOn": "2026-09-01", "book": "business"
        });
        let a = parse_asset(&asset.to_string()).unwrap();
        assert_eq!(a.kind, AssetKind::Inventory);
        assert_eq!(a.value_minor, 150_000);
        assert_eq!(a.note, None);
        assert_eq!(a.book(), Book::Business);
        for (key, value) in [
            ("kind", json!("imovel")),
            ("kind", json!(3)),
            ("valueMinor", json!(12.5)),
            ("valueMinor", json!("10")),
            ("acquiredOn", json!("01/09/2026")),
            ("name", Value::Null),
        ] {
            assert!(
                parse_asset(&with(asset.clone(), key, value)).is_err(),
                "{key}"
            );
        }
        assert!(parse_asset(&without(asset, "valueMinor")).is_err());
    }

    #[test]
    fn divida_valida_e_fora_do_contrato() {
        let debt = json!({
            "name": "Fornecedor", "principalMinor": 500000, "balanceMinor": 320000
        });
        let d = parse_debt(&debt.to_string()).unwrap();
        assert_eq!((d.principal_minor, d.balance_minor), (500_000, 320_000));
        assert_eq!((d.creditor.as_deref(), d.due_on.as_deref()), (None, None));
        assert_eq!(d.book(), Book::Personal);
        let full = with(debt.clone(), "dueOn", json!("2026-12-31"));
        assert_eq!(
            parse_debt(&full).unwrap().due_on.as_deref(),
            Some("2026-12-31")
        );
        for (key, value) in [
            ("balanceMinor", json!("x")),
            ("balanceMinor", json!(1.5)),
            ("dueOn", json!("amanha")),
            ("name", json!(1)),
        ] {
            assert!(
                parse_debt(&with(debt.clone(), key, value)).is_err(),
                "{key}"
            );
        }
        assert!(parse_debt(&without(debt, "principalMinor")).is_err());
    }
}
