//! Lista do mes da tela Lancamentos: filtros, linhas com as referencias resolvidas e grupos
//! por dia. Ordem e agrupamento sao os de `listTransactions` / `groupByDay` do app.

use std::collections::{BTreeMap, BTreeSet};

use super::aggregate::{NO_CATEGORY_COLOR, NO_CATEGORY_NAME, Totals, in_month, sort_desc, totals};
use super::contract::Kind;
use super::dataset::{Dataset, Transaction};
use super::periods::day_heading;

/// Chave de "Sem categoria" no filtro e de "Sem autor" nos chips: string vazia, que nenhum
/// id de 26 caracteres colide.
pub const NONE_KEY: &str = "";

/// Icone de forma de pagamento nula ou apagada.
pub const NO_PAYMENT_ICON: &str = "circle-dashed";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TypeFilter {
    #[default]
    All,
    Income,
    Expense,
}

/// Filtros da tela Lancamentos. Valem so nela e sobrevivem a troca de mes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filters {
    /// `None` = todas; `Some("")` = "Sem categoria" (nula ou apagada); `Some(id)` = uma viva.
    pub category: Option<String>,
    /// Vazio = todos; `""` = sem autor (nulo ou perfil apagado, que colapsam como `findUser`).
    pub authors: BTreeSet<String>,
    pub kind: TypeFilter,
}

impl Filters {
    pub fn is_active(&self) -> bool {
        self.category.is_some() || !self.authors.is_empty() || self.kind != TypeFilter::All
    }

    pub fn clear(&mut self) {
        *self = Filters::default();
    }

    /// Liga ou desliga um chip de autor. Marcar todos os chips da tela (`all`) e o mesmo que
    /// "Todos": o conjunto volta a vazio, como no prototipo, para "Todos" acender e um perfil
    /// novo que chegue por sync ja entrar no filtro.
    pub fn toggle_author(&mut self, key: &str, all: &[String]) {
        if !self.authors.remove(key) {
            self.authors.insert(key.to_string());
        }
        if !all.is_empty() && all.iter().all(|k| self.authors.contains(k)) {
            self.authors.clear();
        }
    }

    fn matches(&self, dataset: &Dataset, tx: &Transaction) -> bool {
        let kind_ok = match self.kind {
            TypeFilter::All => true,
            TypeFilter::Income => tx.kind == Kind::Income,
            TypeFilter::Expense => tx.kind == Kind::Expense,
        };
        let category_ok = self.category.as_deref().is_none_or(|wanted| {
            let actual = dataset
                .find_category(tx.category_id.as_deref())
                .map_or(NONE_KEY, |c| c.id.as_str());
            actual == wanted
        });
        let author_ok = self.authors.is_empty() || {
            let actual = dataset
                .find_user(tx.user_id.as_deref())
                .map_or(NONE_KEY, |u| u.id.as_str());
            self.authors.contains(actual)
        };
        kind_ok && category_ok && author_ok
    }
}

/// Autor visivel de uma linha (perfil vivo).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowAuthor {
    pub id: String,
    pub name: String,
    /// Token de cor do perfil.
    pub color: String,
}

/// Uma linha da tabela (e de "Últimos lançamentos"), com tudo ja resolvido para a tela.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListRow {
    pub id: String,
    pub occurred_on: String,
    pub description: String,
    pub kind: Kind,
    pub amount_minor: i64,
    /// "Sem categoria" (nula) ou "Categoria removida" (apagada), como `resolveCategoryName`.
    pub category_name: String,
    /// Token e icone so de categoria viva; `None` desenha o tile neutro.
    pub category_color: Option<String>,
    pub category_icon: Option<String>,
    pub author: Option<RowAuthor>,
    pub payment_name: String,
    /// Chave de icone do app; `circle-dashed` para forma nula ou apagada.
    pub payment_icon: String,
    /// Tag de recorrencia ("Mensal"...); `None` sem serie.
    pub recurrence_label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DayGroup {
    pub date: String,
    /// "Qui, 24 de setembro".
    pub heading: String,
    /// Receitas menos despesas do dia (das linhas que passaram no filtro).
    pub net_minor: i64,
    pub rows: Vec<ListRow>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ListView {
    pub groups: Vec<DayGroup>,
    pub totals: Totals,
    pub count: usize,
}

pub(crate) fn to_row(dataset: &Dataset, tx: &Transaction) -> ListRow {
    let category = dataset.find_category(tx.category_id.as_deref());
    let payment = dataset.find_payment_method(tx.payment_method_id.as_deref());
    ListRow {
        id: tx.id.clone(),
        occurred_on: tx.occurred_on.clone(),
        description: tx.description.clone(),
        kind: tx.kind,
        amount_minor: tx.amount_minor,
        category_name: dataset.category_name(tx.category_id.as_deref()),
        category_color: category.map(|c| c.color.clone()),
        category_icon: category.map(|c| c.icon.clone()),
        author: dataset.find_user(tx.user_id.as_deref()).map(|u| RowAuthor {
            id: u.id.clone(),
            name: u.name.clone(),
            color: u.color.clone(),
        }),
        payment_name: dataset.payment_method_name(tx.payment_method_id.as_deref()),
        payment_icon: payment.map_or_else(|| NO_PAYMENT_ICON.to_string(), |p| p.icon.clone()),
        recurrence_label: dataset.recurrence_label(tx.recurrence_id.as_deref()),
    }
}

/// O mes inteiro com os filtros, agrupado por dia (sem paginacao: o seletor de mes e a
/// paginacao).
pub fn list_month(dataset: &Dataset, month: &str, filters: &Filters) -> ListView {
    let mut items: Vec<&Transaction> = in_month(dataset, month)
        .filter(|t| filters.matches(dataset, t))
        .collect();
    sort_desc(&mut items);
    let mut groups: Vec<DayGroup> = Vec::new();
    for tx in &items {
        let signed = match tx.kind {
            Kind::Income => tx.amount_minor,
            Kind::Expense => tx.amount_minor.saturating_neg(),
        };
        // Consecutivos do mesmo dia: a lista ja esta ordenada por data.
        match groups.last_mut() {
            Some(group) if group.date == tx.occurred_on => {
                group.net_minor = group.net_minor.saturating_add(signed);
                group.rows.push(to_row(dataset, tx));
            }
            _ => groups.push(DayGroup {
                date: tx.occurred_on.clone(),
                heading: day_heading(&tx.occurred_on),
                net_minor: signed,
                rows: vec![to_row(dataset, tx)],
            }),
        }
    }
    ListView {
        groups,
        totals: totals(items.iter().copied()),
        count: items.len(),
    }
}

/// Uma opcao do menu de categoria. `color` e um token do app, ou `"neutral"` em "Todas", que a
/// tela traduz para `neutral_600`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CategoryOption {
    pub key: Option<String>,
    pub name: String,
    pub color: String,
    pub count: usize,
}

/// Marcador de cor de "Todas as categorias".
pub const ALL_CATEGORIES_COLOR: &str = "neutral";

/// "Todas" (contagem do mes), as categorias vivas com lancamento no mes em ordem alfabetica e
/// "Sem categoria" se houver. Categoria apagada nunca e opcao: os lancamentos dela estao em
/// "Sem categoria", como na rosca.
pub fn category_options(dataset: &Dataset, month: &str) -> Vec<CategoryOption> {
    let mut total = 0;
    let mut without = 0;
    // (nome minusculo, id) -> contagem: ordem alfabetica estavel com desempate por id.
    let mut counts: BTreeMap<(String, String), (String, String, usize)> = BTreeMap::new();
    for tx in in_month(dataset, month) {
        total += 1;
        match dataset.find_category(tx.category_id.as_deref()) {
            Some(c) => {
                counts
                    .entry((c.name.to_lowercase(), c.id.clone()))
                    .or_insert_with(|| (c.name.clone(), c.color.clone(), 0))
                    .2 += 1;
            }
            None => without += 1,
        }
    }
    let mut options = vec![CategoryOption {
        key: None,
        name: "Todas as categorias".into(),
        color: ALL_CATEGORIES_COLOR.into(),
        count: total,
    }];
    options.extend(
        counts
            .into_iter()
            .map(|((_, id), (name, color, count))| CategoryOption {
                key: Some(id),
                name,
                color,
                count,
            }),
    );
    if without > 0 {
        options.push(CategoryOption {
            key: Some(NONE_KEY.into()),
            name: NO_CATEGORY_NAME.into(),
            color: NO_CATEGORY_COLOR.into(),
            count: without,
        });
    }
    options
}

/// O mes tem lancamento sem autor visivel: so entao o chip "Sem autor" aparece.
pub fn has_unauthored(dataset: &Dataset, month: &str) -> bool {
    in_month(dataset, month).any(|t| dataset.find_user(t.user_id.as_deref()).is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::fixtures;

    fn ids(view: &ListView) -> Vec<&str> {
        view.groups
            .iter()
            .flat_map(|g| g.rows.iter().map(|r| r.id.as_str()))
            .collect()
    }

    fn filters(category: Option<&str>, authors: &[&str], kind: TypeFilter) -> Filters {
        Filters {
            category: category.map(Into::into),
            authors: authors.iter().map(|a| a.to_string()).collect(),
            kind,
        }
    }

    #[test]
    fn filtros_ativos_e_limpar() {
        assert!(!Filters::default().is_active());
        assert!(filters(Some("C1"), &[], TypeFilter::All).is_active());
        assert!(filters(Some(""), &[], TypeFilter::All).is_active());
        assert!(filters(None, &["U1"], TypeFilter::All).is_active());
        assert!(filters(None, &[], TypeFilter::Expense).is_active());
        let mut f = filters(Some("C1"), &["U1"], TypeFilter::Income);
        f.clear();
        assert_eq!(f, Filters::default());
    }

    #[test]
    fn marcar_todas_as_pessoas_volta_a_todos() {
        let all = vec!["U1".to_string(), "U2".to_string()];
        let mut f = Filters::default();
        f.toggle_author("U1", &all);
        assert_eq!(f.authors, BTreeSet::from(["U1".to_string()]));
        f.toggle_author("U2", &all);
        assert!(f.authors.is_empty(), "todas marcadas = Todos");
        f.toggle_author("U2", &all);
        f.toggle_author("U2", &all);
        assert!(f.authors.is_empty(), "desmarcar a unica volta a vazio");
        // Com o chip "Sem autor" na tela, ele conta como mais uma opcao.
        let with_none = vec!["U1".to_string(), "U2".to_string(), String::new()];
        f.toggle_author("U1", &with_none);
        f.toggle_author("U2", &with_none);
        assert_eq!(f.authors.len(), 2);
    }

    #[test]
    fn mes_sem_filtro_em_grupos_por_dia() {
        let ds = fixtures();
        let view = list_month(&ds, "2026-09", &Filters::default());
        assert_eq!(view.count, 7);
        assert_eq!(
            ids(&view),
            vec!["T10", "T09", "T08", "T13", "T11", "T07", "T06"]
        );
        let dates: Vec<&str> = view.groups.iter().map(|g| g.date.as_str()).collect();
        assert_eq!(
            dates,
            vec![
                "2026-09-24",
                "2026-09-14",
                "2026-09-12",
                "2026-09-10",
                "2026-09-05"
            ]
        );
        assert_eq!(view.groups[0].heading, "Qui, 24 de setembro");
        assert_eq!(view.groups[0].net_minor, -47_000);
        assert_eq!(view.groups[4].net_minor, 650_000);
        assert_eq!(view.totals.income_minor, 650_000);
        assert_eq!(view.totals.expense_minor, 231_000);
        assert_eq!(view.totals.income_count + view.totals.expense_count, 7);
        assert_eq!(
            list_month(&ds, "2026-05", &Filters::default()),
            ListView::default()
        );
    }

    #[test]
    fn filtro_de_categoria() {
        let ds = fixtures();
        let one = list_month(&ds, "2026-09", &filters(Some("C1"), &[], TypeFilter::All));
        assert_eq!(ids(&one), vec!["T08"]);
        // "Sem categoria" pega a nula e a apagada.
        let none = list_month(&ds, "2026-09", &filters(Some(""), &[], TypeFilter::All));
        assert_eq!(ids(&none), vec!["T10", "T11"]);
        assert_eq!(none.totals.expense_minor, 5_000);
    }

    #[test]
    fn filtro_de_autor_e_tipo() {
        let ds = fixtures();
        let ana = list_month(&ds, "2026-09", &filters(None, &["U1"], TypeFilter::All));
        assert_eq!(ids(&ana), vec!["T10", "T08", "T06"]);
        let both = list_month(
            &ds,
            "2026-09",
            &filters(None, &["U1", "U2"], TypeFilter::All),
        );
        assert_eq!(ids(&both), vec!["T10", "T09", "T08", "T07", "T06"]);
        // Sem autor e autor apagado colapsam.
        let nobody = list_month(&ds, "2026-09", &filters(None, &[""], TypeFilter::All));
        assert_eq!(ids(&nobody), vec!["T13", "T11"]);
        let income = list_month(&ds, "2026-09", &filters(None, &[], TypeFilter::Income));
        assert_eq!(ids(&income), vec!["T06"]);
        let expense = list_month(&ds, "2026-09", &filters(None, &[], TypeFilter::Expense));
        assert_eq!(expense.count, 6);
        // Combinados: Ana + despesa + Alimentacao.
        let combo = list_month(
            &ds,
            "2026-09",
            &filters(Some("C1"), &["U1"], TypeFilter::Expense),
        );
        assert_eq!(ids(&combo), vec!["T08"]);
        let nothing = list_month(
            &ds,
            "2026-09",
            &filters(Some("C1"), &["U2"], TypeFilter::All),
        );
        assert_eq!(nothing.count, 0);
        assert!(nothing.groups.is_empty());
    }

    #[test]
    fn linha_resolve_referencias() {
        let ds = fixtures();
        let view = list_month(&ds, "2026-09", &Filters::default());
        let row = |id: &str| {
            view.groups
                .iter()
                .flat_map(|g| g.rows.iter())
                .find(|r| r.id == id)
                .unwrap()
                .clone()
        };
        let market = row("T08");
        assert_eq!(market.category_name, "Alimentação");
        assert_eq!(market.category_color.as_deref(), Some("orange"));
        assert_eq!(market.category_icon.as_deref(), Some("utensils"));
        assert_eq!(market.author.as_ref().map(|a| a.name.as_str()), Some("Ana"));
        assert_eq!(
            market.author.as_ref().map(|a| a.color.as_str()),
            Some("fuchsia")
        );
        assert_eq!(market.payment_name, "Crédito");
        assert_eq!(market.payment_icon, "credit-card");
        assert_eq!(market.recurrence_label, None);

        let bread = row("T10");
        assert_eq!(bread.category_name, "Sem categoria");
        assert_eq!(bread.category_color, None);
        assert_eq!(bread.category_icon, None);

        let gift = row("T11");
        assert_eq!(gift.category_name, "Categoria removida");
        assert_eq!(gift.category_color, None);
        assert_eq!(gift.author, None);
        assert_eq!(gift.payment_name, "Sem forma de pagamento");
        assert_eq!(gift.payment_icon, "circle-dashed");

        let parking = row("T13");
        assert_eq!(parking.author, None, "autor apagado some");
        assert_eq!(parking.payment_name, "Forma removida");
        assert_eq!(parking.payment_icon, "circle-dashed");

        assert_eq!(row("T06").recurrence_label.as_deref(), Some("Mensal"));
    }

    #[test]
    fn opcoes_de_categoria() {
        let ds = fixtures();
        let options = category_options(&ds, "2026-09");
        let summary: Vec<(Option<&str>, &str, &str, usize)> = options
            .iter()
            .map(|o| (o.key.as_deref(), o.name.as_str(), o.color.as_str(), o.count))
            .collect();
        assert_eq!(
            summary,
            vec![
                (None, "Todas as categorias", "neutral", 7),
                (Some("C1"), "Alimentação", "orange", 1),
                (Some("C2"), "Moradia", "amber", 1),
                (Some("C4"), "Salário", "emerald", 1),
                (Some("C3"), "Transporte", "sky", 2),
                (Some(""), "Sem categoria", "slate", 2),
            ]
        );
        // Agosto: sem "Sem categoria"; categoria apagada nunca e opcao.
        let aug = category_options(&ds, "2026-08");
        assert!(aug.iter().all(|o| o.key.as_deref() != Some("")));
        assert!(aug.iter().all(|o| o.key.as_deref() != Some("C5")));
        // Mes vazio: so "Todas" com zero.
        let may = category_options(&ds, "2026-05");
        assert_eq!(may.len(), 1);
        assert_eq!(may[0].count, 0);
    }

    #[test]
    fn mes_com_lancamento_sem_autor() {
        let ds = fixtures();
        assert!(has_unauthored(&ds, "2026-09"));
        assert!(!has_unauthored(&ds, "2026-08"));
        assert!(!has_unauthored(&ds, "2026-05"));
    }
}
