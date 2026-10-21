//! Metas por categoria contra o gasto de um mes: espelho de `budgets/list.ts` e
//! `budgets/status.ts` do app, para o dashboard do hub mostrar os mesmos numeros do celular.
//!
//! So leitura: o hub nunca edita meta. O gasto e o de `expense_by_category` (despesa viva do mes
//! que conta nos totais, categoria resolvida por `mergedInto`); se a meta usasse outra conta,
//! o hub e o app discordariam sobre quanto se gastou.

use std::collections::HashMap;

use super::aggregate::in_month;
use super::contract::Kind;
use super::dataset::{Budget, Dataset};

/// A partir daqui a meta pede atencao (`BUDGET_WARN_RATIO` do app).
pub const WARN_RATIO: f64 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetLevel {
    Ok,
    /// Perto do limite (a partir de `WARN_RATIO`), inclusive no limite exato.
    Warning,
    /// Passou da meta. Informacao, nao erro: nada bloqueia o gasto.
    Over,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BudgetStatus {
    pub category_id: String,
    pub name: String,
    /// Token de cor do app.
    pub color: String,
    /// Chave de icone do app.
    pub icon: String,
    pub limit_minor: i64,
    pub spent_minor: i64,
    /// `limite - gasto`; negativo quando estourou.
    pub remaining_minor: i64,
    /// `gasto / limite`, sem teto: passa de 1 quando estoura.
    pub used_ratio: f64,
    pub level: BudgetLevel,
    /// Quanto passou da meta; 0 enquanto nao estourou.
    pub over_minor: i64,
    pub count: usize,
}

/// Uma meta por categoria viva (`listBudgets` do app).
///
/// Meta de categoria apagada some. Duas metas para a mesma categoria (aparelhos offline, ou a
/// fusao de uma copia de padrao) valem pela de `updatedAt` maior; o HLC tem largura fixa, entao
/// a comparacao de string e a semantica. Empate (ou sem versao nos dois) desempata pelo id, para
/// o resultado nao depender da ordem do `HashMap`.
pub fn current_budgets(dataset: &Dataset) -> Vec<(&Budget, &str)> {
    let mut by_category: HashMap<&str, &Budget> = HashMap::new();
    for budget in dataset.alive_budgets() {
        let Some(category) = dataset.find_category(Some(&budget.category_id)) else {
            continue;
        };
        let key = category.id.as_str();
        let newer = by_category.get(key).is_none_or(|current| {
            (budget.updated_at.as_deref(), &budget.id)
                > (current.updated_at.as_deref(), &current.id)
        });
        if newer {
            by_category.insert(key, budget);
        }
    }
    by_category.into_iter().map(|(id, b)| (b, id)).collect()
}

fn level_of(spent: i64, limit: i64) -> BudgetLevel {
    if spent > limit {
        BudgetLevel::Over
    } else if spent as f64 >= limit as f64 * WARN_RATIO {
        BudgetLevel::Warning
    } else {
        BudgetLevel::Ok
    }
}

/// Metas do mes `month` (`YYYY-MM`), da mais perto (ou alem) do limite para a mais longe; empate
/// pelo nome.
pub fn budget_report(dataset: &Dataset, month: &str) -> Vec<BudgetStatus> {
    let mut spent: HashMap<&str, (i64, usize)> = HashMap::new();
    for tx in in_month(dataset, month).filter(|t| t.kind == Kind::Expense && t.counts_in_totals()) {
        let Some(category) = dataset.find_category(tx.category_id.as_deref()) else {
            continue;
        };
        let entry = spent.entry(category.id.as_str()).or_default();
        entry.0 = entry.0.saturating_add(tx.amount_minor);
        entry.1 += 1;
    }

    let mut rows: Vec<BudgetStatus> = current_budgets(dataset)
        .into_iter()
        .filter_map(|(budget, category_id)| {
            let category = dataset.find_category(Some(category_id))?;
            let (spent_minor, count) = spent.get(category_id).copied().unwrap_or_default();
            Some(BudgetStatus {
                category_id: category.id.clone(),
                name: category.name.clone(),
                color: category.color.clone(),
                icon: category.icon.clone(),
                limit_minor: budget.limit_minor,
                spent_minor,
                remaining_minor: budget.limit_minor.saturating_sub(spent_minor),
                used_ratio: spent_minor as f64 / budget.limit_minor as f64,
                level: level_of(spent_minor, budget.limit_minor),
                over_minor: spent_minor.saturating_sub(budget.limit_minor).max(0),
                count,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        b.used_ratio
            .total_cmp(&a.used_ratio)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.category_id.cmp(&b.category_id))
    });
    rows
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::dashboard::dataset::{BUDGETS, CATEGORIES, RawRow, TRANSACTIONS};

    struct Seed {
        seq: i64,
        rows: Vec<RawRow>,
    }

    impl Seed {
        fn new() -> Self {
            Self {
                seq: 0,
                rows: Vec::new(),
            }
        }

        fn push(&mut self, table: &str, id: &str, deleted: bool, data: Value) -> &mut Self {
            self.seq += 1;
            self.rows.push(RawRow {
                table: table.into(),
                id: id.into(),
                deleted_at: deleted.then(|| "2026-09-30T00:00:00Z".to_string()),
                seq: self.seq,
                data: data.to_string(),
            });
            self
        }

        fn category(&mut self, id: &str, name: &str) -> &mut Self {
            self.push(
                CATEGORIES,
                id,
                false,
                json!({ "name": name, "color": "sky", "icon": "tag" }),
            )
        }

        fn budget(&mut self, id: &str, category: &str, limit: i64, version: &str) -> &mut Self {
            self.push(
                BUDGETS,
                id,
                false,
                json!({ "categoryId": category, "limitMinor": limit, "updatedAt": version }),
            )
        }

        fn expense(
            &mut self,
            id: &str,
            amount: i64,
            on: &str,
            category: Option<&str>,
        ) -> &mut Self {
            self.push(
                TRANSACTIONS,
                id,
                false,
                json!({
                    "kind": "expense", "description": id, "amountMinor": amount,
                    "occurredOn": on, "categoryId": category
                }),
            )
        }

        fn dataset(&self) -> Dataset {
            let mut ds = Dataset::default();
            ds.apply(self.rows.clone());
            ds
        }
    }

    const MONTH: &str = "2026-09";

    #[test]
    fn gasto_restante_uso_e_lancamentos() {
        let mut s = Seed::new();
        s.category("FOOD", "Alimentação")
            .budget("B1", "FOOD", 100_000, "1-A")
            .expense("a", 30_000, "2026-09-02", Some("FOOD"))
            .expense("b", 25_000, "2026-09-09", Some("FOOD"))
            .expense("c", 99_000, "2026-08-30", Some("FOOD"));
        let rows = budget_report(&s.dataset(), MONTH);
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(
            (r.limit_minor, r.spent_minor, r.remaining_minor, r.count),
            (100_000, 55_000, 45_000, 2)
        );
        assert!((r.used_ratio - 0.55).abs() < 1e-9);
        assert_eq!((r.level, r.over_minor), (BudgetLevel::Ok, 0));
    }

    #[test]
    fn niveis_ok_atencao_e_estouro() {
        let mut s = Seed::new();
        s.category("A", "A")
            .category("B", "B")
            .category("C", "C")
            .category("D", "D")
            .budget("BA", "A", 10_000, "1-A")
            .budget("BB", "B", 10_000, "1-A")
            .budget("BC", "C", 10_000, "1-A")
            .budget("BD", "D", 10_000, "1-A")
            .expense("a", 7_900, "2026-09-02", Some("A"))
            .expense("b", 8_000, "2026-09-02", Some("B"))
            .expense("c", 10_000, "2026-09-02", Some("C"))
            .expense("d", 12_500, "2026-09-02", Some("D"));
        let rows = budget_report(&s.dataset(), MONTH);
        let level = |id: &str| rows.iter().find(|r| r.category_id == id).unwrap();
        assert_eq!(level("A").level, BudgetLevel::Ok);
        assert_eq!(level("B").level, BudgetLevel::Warning);
        // No limite exato: atencao, nao estouro.
        assert_eq!(
            (level("C").level, level("C").over_minor),
            (BudgetLevel::Warning, 0)
        );
        assert_eq!(
            (
                level("D").level,
                level("D").over_minor,
                level("D").remaining_minor
            ),
            (BudgetLevel::Over, 2_500, -2_500)
        );
        // Mais perto do limite primeiro.
        let order: Vec<&str> = rows.iter().map(|r| r.category_id.as_str()).collect();
        assert_eq!(order, ["D", "C", "B", "A"]);
    }

    #[test]
    fn transferencia_e_oculto_ficam_fora_do_gasto() {
        let mut s = Seed::new();
        s.category("FOOD", "Alimentação")
            .budget("B1", "FOOD", 100_000, "1-A")
            .expense("a", 5_000, "2026-09-04", Some("FOOD"));
        for (id, state) in [("t", "transfer"), ("h", "hidden")] {
            s.push(
                TRANSACTIONS,
                id,
                false,
                json!({
                    "kind": "expense", "description": id, "amountMinor": 80_000,
                    "occurredOn": "2026-09-03", "categoryId": "FOOD", "reportState": state
                }),
            );
        }
        let rows = budget_report(&s.dataset(), MONTH);
        assert_eq!((rows[0].spent_minor, rows[0].count), (5_000, 1));
    }

    #[test]
    fn receita_nao_conta_e_lancamento_apagado_tambem_nao() {
        let mut s = Seed::new();
        s.category("FOOD", "Alimentação")
            .budget("B1", "FOOD", 100_000, "1-A")
            .push(
                TRANSACTIONS,
                "inc",
                false,
                json!({
                    "kind": "income", "description": "x", "amountMinor": 90_000,
                    "occurredOn": "2026-09-03", "categoryId": "FOOD"
                }),
            )
            .push(
                TRANSACTIONS,
                "del",
                true,
                json!({
                    "kind": "expense", "description": "x", "amountMinor": 90_000,
                    "occurredOn": "2026-09-03", "categoryId": "FOOD"
                }),
            );
        assert_eq!(budget_report(&s.dataset(), MONTH)[0].spent_minor, 0);
    }

    #[test]
    fn meta_de_categoria_apagada_ou_inexistente_some() {
        let mut s = Seed::new();
        s.category("FOOD", "Alimentação")
            .push(
                CATEGORIES,
                "OLD",
                true,
                json!({ "name": "Velha", "color": "sky", "icon": "tag" }),
            )
            .budget("B1", "FOOD", 10_000, "1-A")
            .budget("B2", "OLD", 10_000, "1-A")
            .budget("B3", "GONE", 10_000, "1-A");
        let rows = budget_report(&s.dataset(), MONTH);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].category_id, "FOOD");
    }

    #[test]
    fn meta_apagada_some() {
        let mut s = Seed::new();
        s.category("FOOD", "Alimentação").push(
            BUDGETS,
            "B1",
            true,
            json!({ "categoryId": "FOOD", "limitMinor": 10_000, "updatedAt": "1-A" }),
        );
        assert!(budget_report(&s.dataset(), MONTH).is_empty());
    }

    #[test]
    fn duas_metas_da_mesma_categoria_valem_pela_de_updated_at_maior() {
        let mut s = Seed::new();
        s.category("FOOD", "Alimentação")
            .budget("NEW", "FOOD", 20_000, "1754568000005-0000-AAA")
            .budget("OLD", "FOOD", 10_000, "1754568000000-0000-AAA");
        let rows = budget_report(&s.dataset(), MONTH);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].limit_minor, 20_000);
    }

    #[test]
    fn meta_sem_versao_perde_para_a_com_versao() {
        let mut s = Seed::new();
        s.category("FOOD", "Alimentação")
            .push(
                BUDGETS,
                "ZZZ",
                false,
                json!({ "categoryId": "FOOD", "limitMinor": 99_000 }),
            )
            .budget("AAA", "FOOD", 20_000, "1754568000000-0000-AAA");
        assert_eq!(budget_report(&s.dataset(), MONTH)[0].limit_minor, 20_000);
    }

    #[test]
    fn categoria_fundida_reune_gasto_e_metas() {
        let mut s = Seed::new();
        s.category("STABLE", "Alimentação")
            .push(
                CATEGORIES,
                "COPY",
                true,
                json!({ "name": "Alimentação", "color": "sky", "icon": "tag", "mergedInto": "STABLE" }),
            )
            .budget("B-COPY", "COPY", 10_000, "1754568000009-0000-AAA")
            .budget("B-STABLE", "STABLE", 30_000, "1754568000001-0000-AAA")
            .expense("a", 4_000, "2026-09-02", Some("COPY"))
            .expense("b", 1_000, "2026-09-02", Some("STABLE"));
        let rows = budget_report(&s.dataset(), MONTH);
        assert_eq!(rows.len(), 1);
        // A meta da copia e mais nova: vale ela, apontando para a categoria estavel.
        assert_eq!(
            (
                rows[0].category_id.as_str(),
                rows[0].limit_minor,
                rows[0].spent_minor
            ),
            ("STABLE", 10_000, 5_000)
        );
    }

    #[test]
    fn gasto_em_categoria_sem_meta_nao_entra() {
        let mut s = Seed::new();
        s.category("FOOD", "Alimentação")
            .category("PET", "Pet")
            .budget("B1", "FOOD", 10_000, "1-A")
            .expense("a", 5_000, "2026-09-02", Some("PET"))
            .expense("b", 5_000, "2026-09-02", None);
        let rows = budget_report(&s.dataset(), MONTH);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].spent_minor, 0);
    }

    #[test]
    fn sem_metas_nada() {
        assert!(budget_report(&Dataset::default(), MONTH).is_empty());
    }

    #[test]
    fn linha_de_meta_fora_do_contrato_e_ignorada_e_contada() {
        let mut s = Seed::new();
        s.category("FOOD", "Alimentação")
            .push(
                BUDGETS,
                "B1",
                false,
                json!({ "categoryId": "FOOD", "limitMinor": 0 }),
            )
            .push(BUDGETS, "B2", false, json!({ "limitMinor": 5 }));
        let ds = s.dataset();
        assert!(budget_report(&ds, MONTH).is_empty());
        assert_eq!(ds.ignored_total(), 2);
        assert_eq!(ds.ignored[BUDGETS].len(), 2);
    }
}
