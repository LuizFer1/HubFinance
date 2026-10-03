//! Modelos de tela: tudo o que Dashboard e Lancamentos mostram, derivado do `Dataset` por
//! funcoes puras. A UI chama `build_*` so quando dados, mes, filtros ou "hoje" mudam e guarda
//! o resultado; desenhar um quadro nunca percorre o dataset.

use super::aggregate::{
    MonthSummary, MonthTotals, NO_CATEGORY_COLOR, Slice, average_surplus, axis_top,
    expense_by_category, month_summary, monthly_totals, recent,
};
use super::dataset::Dataset;
use super::list::{
    CategoryOption, Filters, ListRow, ListView, NONE_KEY, category_options, has_unauthored,
    list_month, to_row,
};
use super::periods::{last_months, month_label_capitalized, month_of, month_short, partial_label};

/// Meses das barras de "Receita × despesa": seis terminando no mes escolhido.
pub const BAR_MONTHS: usize = 6;
/// Linhas de "Últimos lançamentos".
pub const RECENT_ROWS: usize = 6;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DashboardView {
    pub month: String,
    /// "Setembro 2026" (seletor de mes).
    pub month_label: String,
    /// O mes escolhido e o de hoje: os dados ainda podem crescer.
    pub is_current: bool,
    /// "24 set" da tag "Parcial · até 24 set"; so no mes corrente.
    pub partial_until: Option<String>,
    pub summary: MonthSummary,
    pub donut: Vec<Slice>,
    /// Seis meses, o mais antigo primeiro; o ultimo e o escolhido.
    pub bars: Vec<MonthTotals>,
    pub axis_top: i64,
    pub average_surplus: i64,
    /// "entre abr e set".
    pub bars_range_label: String,
    pub recent: Vec<ListRow>,
    /// Hub sem nenhum lancamento vivo: o estado "Sem dados" (nao confundir com mes vazio).
    pub is_empty: bool,
    pub ignored_total: usize,
}

/// `today` e `YYYY-MM-DD` no fuso local, lido pela UI; `month` e `YYYY-MM`.
pub fn build_dashboard(dataset: &Dataset, today: &str, month: &str) -> DashboardView {
    let is_current = month == month_of(today);
    let bars = monthly_totals(dataset, &last_months(month, BAR_MONTHS));
    let peak = bars
        .iter()
        .map(|b| b.income_minor.max(b.expense_minor))
        .max()
        .unwrap_or(0);
    let bars_range_label = match (bars.first(), bars.last()) {
        (Some(first), Some(last)) => format!(
            "entre {} e {}",
            month_short(&first.month),
            month_short(&last.month)
        ),
        _ => String::new(),
    };
    DashboardView {
        month: month.to_string(),
        month_label: month_label_capitalized(month),
        is_current,
        partial_until: is_current.then(|| partial_label(today)),
        summary: month_summary(dataset, month),
        donut: expense_by_category(dataset, month),
        axis_top: axis_top(peak),
        average_surplus: average_surplus(&bars),
        bars,
        bars_range_label,
        recent: recent(dataset, month, RECENT_ROWS)
            .into_iter()
            .map(|tx| to_row(dataset, tx))
            .collect(),
        is_empty: dataset.is_empty(),
        ignored_total: dataset.ignored_total(),
    }
}

/// Um chip de autor: perfil vivo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorOption {
    pub id: String,
    pub name: String,
    /// Token de cor do perfil.
    pub color: String,
    /// Data URI da foto; a UI decodifica uma vez por perfil.
    pub avatar_uri: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransactionsView {
    pub month: String,
    pub month_label: String,
    pub list: ListView,
    /// Opcoes do menu de categoria.
    pub categories: Vec<CategoryOption>,
    /// Rotulo e cor do botao de categoria quando ha filtro. Resolvido no dataset, nao nas
    /// opcoes do mes: o filtro sobrevive a troca de mes e a categoria escolhida pode nao ter
    /// lancamento no mes novo.
    pub selected_category: Option<CategoryOption>,
    /// Perfis vivos em ordem alfabetica.
    pub authors: Vec<AuthorOption>,
    /// Mostrar o chip "Sem autor": o mes tem lancamento sem autor visivel, ou o chip esta
    /// marcado (sumir com ele marcado deixaria um filtro invisivel).
    pub unauthored_chip: bool,
    pub is_empty_hub: bool,
    /// Aparelhos ativos que ja enviaram dados. Vem do snapshot, nao do dataset: a UI preenche
    /// (aqui fica 0).
    pub synced_devices: usize,
}

pub fn build_transactions(
    dataset: &Dataset,
    // Sem uso por enquanto: fica na assinatura para as duas telas nascerem do mesmo par
    // (hoje, mes) e uma regra de "mes corrente" na lista nao exigir mudar quem chama.
    _today: &str,
    month: &str,
    filters: &Filters,
) -> TransactionsView {
    TransactionsView {
        month: month.to_string(),
        month_label: month_label_capitalized(month),
        list: list_month(dataset, month, filters),
        categories: category_options(dataset, month),
        selected_category: filters
            .category
            .as_deref()
            .map(|key| selected_category(dataset, key)),
        authors: dataset
            .alive_users()
            .into_iter()
            .map(|u| AuthorOption {
                id: u.id.clone(),
                name: u.name.clone(),
                color: u.color.clone(),
                avatar_uri: u.avatar.clone(),
            })
            .collect(),
        unauthored_chip: has_unauthored(dataset, month) || filters.authors.contains(NONE_KEY),
        is_empty_hub: dataset.is_empty(),
        synced_devices: 0,
    }
}

fn selected_category(dataset: &Dataset, key: &str) -> CategoryOption {
    let (name, color) = if key == NONE_KEY {
        (dataset.category_name(None), NO_CATEGORY_COLOR.to_string())
    } else {
        match dataset.find_category(Some(key)) {
            Some(c) => (c.name.clone(), c.color.clone()),
            None => (
                dataset.category_name(Some(key)),
                NO_CATEGORY_COLOR.to_string(),
            ),
        }
    };
    CategoryOption {
        key: Some(key.to_string()),
        name,
        color,
        count: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::fixtures;
    use crate::dashboard::list::{TypeFilter, category_options, list_month};

    #[test]
    fn dashboard_do_mes_corrente() {
        let ds = fixtures();
        let v = build_dashboard(&ds, "2026-09-24", "2026-09");
        assert_eq!(v.month, "2026-09");
        assert!(v.is_current);
        assert_eq!(v.partial_until.as_deref(), Some("24 set"));
        assert_eq!(v.month_label, "Setembro 2026");
        assert_eq!(v.bars.len(), 6);
        assert_eq!(v.bars.last().map(|b| b.month.as_str()), Some("2026-09"));
        let peak = v
            .bars
            .iter()
            .map(|b| b.income_minor.max(b.expense_minor))
            .max()
            .unwrap();
        assert!(v.axis_top >= peak);
        assert_eq!(v.axis_top, 800_000);
        assert_eq!(v.bars_range_label, "entre abr e set");
        // (6500 - 1800) + (6500 - 2100) + (6500 - 2310) em seis meses.
        assert_eq!(v.average_surplus, 221_500);
        assert_eq!(v.recent.len(), 6);
        assert_eq!(v.recent[0].id, "T10");
        assert!(!v.is_empty);
        assert_eq!(v.donut.len(), 4);
        assert_eq!(v.summary.current.expense_count, 6);
        assert_eq!(v.ignored_total, 0);
    }

    #[test]
    fn dashboard_de_mes_passado() {
        let ds = fixtures();
        let v = build_dashboard(&ds, "2026-09-24", "2026-08");
        assert!(!v.is_current);
        assert_eq!(v.partial_until, None);
        assert_eq!(v.bars_range_label, "entre mar e ago");
        assert_eq!(v.bars.last().map(|b| b.month.as_str()), Some("2026-08"));
    }

    #[test]
    fn dashboard_na_virada_de_ano() {
        let ds = fixtures();
        let v = build_dashboard(&ds, "2027-01-03", "2027-01");
        assert_eq!(v.bars_range_label, "entre ago e jan");
        assert_eq!(v.bars[0].month, "2026-08");
        assert_eq!(v.partial_until.as_deref(), Some("3 jan"));
        assert!(v.donut.is_empty());
        assert!(v.recent.is_empty());
        // O hub tem dados; o mes e que esta vazio.
        assert!(!v.is_empty);
    }

    #[test]
    fn dashboard_sem_dados() {
        let v = build_dashboard(&Dataset::default(), "2026-09-24", "2026-09");
        assert!(v.is_empty);
        assert_eq!(v.summary, MonthSummary::default());
        assert!(v.donut.is_empty());
        assert!(v.recent.is_empty());
        assert!(
            v.bars
                .iter()
                .all(|b| b.income_minor == 0 && b.expense_minor == 0)
        );
        assert_eq!(v.axis_top, 200_000);
        assert_eq!(v.average_surplus, 0);
    }

    #[test]
    fn dashboard_conta_as_ignoradas() {
        use crate::dashboard::dataset::{RawRow, TRANSACTIONS};
        let mut ds = fixtures();
        ds.apply([RawRow {
            table: TRANSACTIONS.into(),
            id: "TX".into(),
            deleted_at: None,
            seq: 999,
            data: serde_json::json!({ "kind": "expense" }).to_string(),
        }]);
        assert_eq!(
            build_dashboard(&ds, "2026-09-24", "2026-09").ignored_total,
            1
        );
    }

    #[test]
    fn lancamentos_com_filtros() {
        let ds = fixtures();
        let filters = Filters {
            authors: ["U1".to_string()].into(),
            ..Filters::default()
        };
        let v = build_transactions(&ds, "2026-09-24", "2026-09", &filters);
        assert_eq!(v.month, "2026-09");
        assert_eq!(v.month_label, "Setembro 2026");
        assert_eq!(v.list, list_month(&ds, "2026-09", &filters));
        assert_eq!(v.list.count, 3);
        assert_eq!(v.categories, category_options(&ds, "2026-09"));
        let authors: Vec<(&str, &str, &str)> = v
            .authors
            .iter()
            .map(|a| (a.id.as_str(), a.name.as_str(), a.color.as_str()))
            .collect();
        assert_eq!(
            authors,
            vec![("U1", "Ana", "fuchsia"), ("U2", "Luiz", "sky")]
        );
        assert!(v.unauthored_chip);
        assert!(!v.is_empty_hub);
        assert_eq!(v.synced_devices, 0);
        assert_eq!(v.selected_category, None);
    }

    #[test]
    fn categoria_escolhida_mesmo_fora_do_mes() {
        let ds = fixtures();
        let pick = |category: &str| Filters {
            category: Some(category.into()),
            ..Filters::default()
        };
        // Alimentacao nao tem lancamento em julho, mas o botao continua dizendo o nome.
        let v = build_transactions(&ds, "2026-09-24", "2026-07", &pick("C1"));
        let selected = v.selected_category.unwrap();
        assert_eq!(selected.name, "Alimentação");
        assert_eq!(selected.color, "orange");
        assert_eq!(v.list.count, 0);
        let none = build_transactions(&ds, "2026-09-24", "2026-09", &pick(""));
        assert_eq!(none.selected_category.unwrap().name, "Sem categoria");
        // Categoria apagada depois de escolhida.
        let dead = build_transactions(&ds, "2026-09-24", "2026-09", &pick("C5"));
        assert_eq!(dead.selected_category.unwrap().name, "Categoria removida");
        // Chip "Sem autor" fica enquanto estiver marcado, mesmo num mes sem esse caso.
        let filters = Filters {
            authors: [String::new()].into(),
            kind: TypeFilter::All,
            ..Filters::default()
        };
        assert!(build_transactions(&ds, "2026-09-24", "2026-08", &filters).unauthored_chip);
        assert!(
            !build_transactions(&ds, "2026-09-24", "2026-08", &Filters::default()).unauthored_chip
        );
    }

    /// Regra n. 1 do handoff ("guardar nao e gastar"): movimentacao de reserva nao e receita
    /// nem despesa. Se alguem um dia somar `reserve_movements` em `aggregate.rs` ou `list.rs`,
    /// e este teste que quebra.
    #[test]
    fn reservas_nao_entram_no_dashboard_nem_nos_lancamentos() {
        use crate::dashboard::fixtures_with_reserves;
        use crate::dashboard::periods::month_window;
        let plain = fixtures();
        let with = fixtures_with_reserves();
        assert_eq!(
            plain.alive_transactions().count(),
            with.alive_transactions().count()
        );
        assert_eq!(plain.is_empty(), with.is_empty());
        for month in month_window("2026-09-24") {
            assert_eq!(
                build_dashboard(&plain, "2026-09-24", &month),
                build_dashboard(&with, "2026-09-24", &month),
                "{month}"
            );
            assert_eq!(
                build_transactions(&plain, "2026-09-24", &month, &Filters::default()),
                build_transactions(&with, "2026-09-24", &month, &Filters::default()),
                "{month}"
            );
        }
        assert!(with.has_reserves() && !plain.has_reserves());
    }

    #[test]
    fn lancamentos_com_hub_vazio() {
        let v = build_transactions(
            &Dataset::default(),
            "2026-09-24",
            "2026-09",
            &Filters::default(),
        );
        assert!(v.is_empty_hub);
        assert_eq!(v.list.count, 0);
        assert!(v.authors.is_empty());
        assert!(!v.unauthored_chip);
    }
}
