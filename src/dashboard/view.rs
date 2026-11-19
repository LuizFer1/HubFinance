//! Modelos de tela: tudo o que Lancamentos e Reservas mostram (o Dashboard e o `grafite`), derivado do `Dataset` por
//! funcoes puras. A UI chama `build_*` so quando dados, mes, filtros ou "hoje" mudam e guarda
//! o resultado; desenhar um quadro nunca percorre o dataset.

use super::aggregate::NO_CATEGORY_COLOR;
use super::contract::ReserveKind;
use super::dataset::Dataset;
use super::list::{
    CategoryOption, Filters, ListView, NONE_KEY, category_options, has_unauthored, list_month,
};
use super::periods::{month_label_capitalized, month_of, month_window};
use super::reserves::{
    EmergencyView, MovementRow, PotRow, ReserveSeries, ReserveShare, ReserveTotals, emergency_goal,
    emergency_view, movements, ordered, pot_row, pots_note, reserve_axis_top, series, shares,
    totals, withdrawals_note,
};

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

/// Tela Reservas. Sem mes escolhido: saldos sao acumulados e a janela e sempre a de `today`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReservesView {
    /// Nenhuma reserva viva: o estado vazio. Reservas vivas sem movimentacao mostram os cards
    /// normais zerados.
    pub is_empty: bool,
    pub reserve_count: usize,
    pub current_month: String,
    /// 12, a janela, o mais antigo primeiro.
    pub months: Vec<String>,
    pub totals: ReserveTotals,
    pub withdrawals_note: String,
    /// Barra dividida do card Total: saldo > 0, ordem padrao.
    pub shares: Vec<ReserveShare>,
    pub emergency: Option<EmergencyView>,
    pub pots: Vec<PotRow>,
    pub pots_note: String,
    pub series: Vec<ReserveSeries>,
    pub axis_top: i64,
    pub movements: Vec<MovementRow>,
    pub movement_count: usize,
}

pub fn build_reserves(dataset: &Dataset, today: &str) -> ReservesView {
    let current = month_of(today);
    let months = month_window(today);
    let (emergency, rest) = ordered(dataset);
    let pots: Vec<PotRow> = rest
        .iter()
        .map(|r| {
            // Emergencia extra (nao virou o card): meta derivada das proprias categorias.
            let derived =
                (r.kind == ReserveKind::Emergency).then(|| emergency_goal(dataset, r, &current));
            pot_row(dataset, r, &current, derived)
        })
        .collect();
    let series = series(dataset, &months);
    let peak = (0..months.len())
        .map(|i| {
            series
                .iter()
                .filter_map(|s| s.balances.get(i))
                .fold(0i64, |acc, v| acc.saturating_add(*v))
        })
        .max()
        .unwrap_or(0);
    let totals = totals(dataset, &months);
    let (movements, movement_count) = movements(dataset, &months);
    ReservesView {
        is_empty: !dataset.has_reserves(),
        reserve_count: dataset.reserve_count(),
        withdrawals_note: withdrawals_note(&totals),
        totals,
        shares: shares(dataset, &current),
        emergency: emergency.map(|r| emergency_view(dataset, r, &current)),
        pots_note: pots_note(&pots),
        pots,
        axis_top: reserve_axis_top(peak),
        series,
        movements,
        movement_count,
        current_month: current,
        months,
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
        let v = crate::dashboard::grafite::build_grafite(
            &ds,
            "2026-09-24",
            "2026-09",
            crate::dashboard::contract::Book::Personal,
            false,
        );
        assert_eq!(v.ignored_total, 1);
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
    /// O lado do Dashboard esta em `grafite::movimentos_de_reserva_so_mudam_o_card_da_reserva`.
    fn reservas_nao_entram_nos_lancamentos() {
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
                build_transactions(&plain, "2026-09-24", &month, &Filters::default()),
                build_transactions(&with, "2026-09-24", &month, &Filters::default()),
                "{month}"
            );
        }
        assert!(with.has_reserves() && !plain.has_reserves());
    }

    #[test]
    fn reservas_da_fixture() {
        use crate::dashboard::fixtures_with_reserves;
        let v = build_reserves(&fixtures_with_reserves(), "2026-09-24");
        assert!(!v.is_empty);
        assert_eq!(v.reserve_count, 6);
        assert_eq!(v.current_month, "2026-09");
        assert_eq!(v.months.len(), 12);
        assert_eq!(v.totals.total_minor, 603_000);
        assert_eq!(v.withdrawals_note, "2 retiradas de 2 reservas");
        assert_eq!(v.shares.len(), 6);
        assert_eq!(v.emergency.as_ref().map(|e| e.goal_minor), Some(390_000));
        let pots: Vec<&str> = v.pots.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(pots, vec!["RP1", "RP2", "RP3", "RP4", "RP6"]);
        assert_eq!(v.pots_note, "4 com objetivo · 1 sem meta");
        assert_eq!(v.series.len(), 6);
        assert_eq!(v.axis_top, 1_200_000);
        assert_eq!(v.movements.len(), 8);
        assert_eq!(v.movement_count, 15);
    }

    #[test]
    fn reservas_sem_reservas() {
        for ds in [fixtures(), Dataset::default()] {
            let v = build_reserves(&ds, "2026-09-24");
            assert!(v.is_empty);
            assert_eq!(v.reserve_count, 0);
            assert_eq!(v.totals, ReserveTotals::default());
            assert!(v.shares.is_empty() && v.pots.is_empty() && v.series.is_empty());
            assert!(v.movements.is_empty());
            assert_eq!(v.movement_count, 0);
            assert_eq!(v.emergency, None);
            assert_eq!(v.axis_top, 600_000);
        }
    }

    #[test]
    fn reservas_vivas_sem_movimentacao() {
        use crate::dashboard::dataset::{RESERVES, RawRow};
        let mut ds = fixtures();
        ds.apply([RawRow {
            table: RESERVES.into(),
            id: "RP1".into(),
            deleted_at: None,
            seq: 500,
            data: serde_json::json!({ "kind": "goal", "name": "X", "icon": "tag", "color": "sky" })
                .to_string(),
        }]);
        let v = build_reserves(&ds, "2026-09-24");
        assert!(!v.is_empty);
        assert_eq!(v.totals.total_minor, 0);
        assert!(v.shares.is_empty());
        assert_eq!(v.pots.len(), 1);
    }

    #[test]
    fn reservas_com_duas_emergencias() {
        use crate::dashboard::dataset::{RESERVES, RawRow};
        use crate::dashboard::fixtures_with_reserves;
        use crate::dashboard::reserves::ReserveColor;
        let mut ds = fixtures_with_reserves();
        ds.apply([RawRow {
            table: RESERVES.into(),
            id: "RE0".into(),
            deleted_at: None,
            seq: 500,
            data: serde_json::json!({ "kind": "emergency", "name": "Primeira", "icon": "x",
                "color": "slate", "multiple": 3, "essentialCategoryIds": ["C2"] })
            .to_string(),
        }]);
        let v = build_reserves(&ds, "2026-09-24");
        assert_eq!(v.emergency.as_ref().map(|e| e.id.as_str()), Some("RE0"));
        let extra = &v.pots[0];
        assert_eq!(extra.id, "RE1");
        assert_eq!(extra.color, ReserveColor::Accent);
        // A meta da extra e a dela: 6 x 65.000.
        assert_eq!(extra.goal_minor, Some(390_000));
    }

    #[test]
    fn reservas_na_virada_de_ano() {
        use crate::dashboard::fixtures_with_reserves;
        let v = build_reserves(&fixtures_with_reserves(), "2027-01-03");
        assert_eq!(v.months[0], "2026-02");
        assert_eq!(v.current_month, "2027-01");
        // `M06` (outubro de 2026) ja passou e entra.
        assert_eq!(v.emergency.as_ref().map(|e| e.balance_minor), Some(240_000));
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
