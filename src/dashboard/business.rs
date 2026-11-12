//! Painel "Negocio" da conta comercial: resultado do mes, capital aplicado por tipo de bem e
//! dividas totais, tudo puro e a partir do `Dataset`. Espelha as projecoes do app
//! (`capitalByKind`, `debtTotal`) para os dois mostrarem o mesmo numero.
//!
//! Bens e dividas NUNCA entram em receita nem despesa: o resultado do mes sai so de
//! `transactions` (via `aggregate`), e esta soma so le `assets` e `debts`. O teste
//! `bens_e_dividas_nao_mudam_receita_nem_despesa` fixa isso.

use super::aggregate::{in_month, totals};
use super::contract::{AssetKind, Book};
use super::dataset::Dataset;

/// Numeros do bloco "Negocio" de um mes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BusinessPanel {
    /// Receita, despesa e resultado do mes (mesma regra do resumo: transferencia e oculto
    /// ficam de fora).
    pub income_minor: i64,
    pub expense_minor: i64,
    pub result_minor: i64,
    /// Capital por tipo de bem, sem as linhas apagadas.
    pub investment_minor: i64,
    pub inventory_minor: i64,
    pub equipment_minor: i64,
    /// Investimento + estoque + equipamento.
    pub capital_minor: i64,
    /// Soma de `balanceMinor` das dividas vivas (o saldo devedor atual, nao o principal).
    pub debt_minor: i64,
}

/// Soma dos bens vivos de um tipo. Nao tem recorte de mes: capital e o que o negocio tem
/// agora, como no app.
pub fn capital_of(dataset: &Dataset, kind: AssetKind) -> i64 {
    dataset
        .assets
        .values()
        .filter(|a| !a.deleted && a.book == Book::Business && a.kind == kind)
        .fold(0i64, |acc, a| acc.saturating_add(a.value_minor))
}

/// Saldo devedor total das dividas vivas da conta comercial.
pub fn debt_total(dataset: &Dataset) -> i64 {
    dataset
        .debts
        .values()
        .filter(|d| !d.deleted && d.book == Book::Business)
        .fold(0i64, |acc, d| acc.saturating_add(d.balance_minor))
}

/// O bloco de `month` (`YYYY-MM`). Quem chama passa a visao da conta comercial
/// (`Dataset::for_book(Book::Business)`); o filtro por `book` aqui e redundante de proposito,
/// para uma visao errada (a pessoal) dar zero em vez de somar bem pessoal como capital.
pub fn business_panel(dataset: &Dataset, month: &str) -> BusinessPanel {
    let month_totals = totals(in_month(dataset, month).filter(|t| t.book == Book::Business));
    let investment_minor = capital_of(dataset, AssetKind::Investment);
    let inventory_minor = capital_of(dataset, AssetKind::Inventory);
    let equipment_minor = capital_of(dataset, AssetKind::Equipment);
    BusinessPanel {
        income_minor: month_totals.income_minor,
        expense_minor: month_totals.expense_minor,
        result_minor: month_totals.balance_minor,
        investment_minor,
        inventory_minor,
        equipment_minor,
        capital_minor: investment_minor
            .saturating_add(inventory_minor)
            .saturating_add(equipment_minor),
        debt_minor: debt_total(dataset),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::dashboard::dataset::{ASSETS, DEBTS, RawRow, TRANSACTIONS};

    fn raw(table: &str, id: &str, deleted: bool, seq: i64, data: Value) -> RawRow {
        RawRow {
            table: table.into(),
            id: id.into(),
            deleted_at: deleted.then(|| "2026-09-30T00:00:00Z".to_string()),
            seq,
            data: data.to_string(),
        }
    }

    fn asset(kind: &str, value: i64, book: &str) -> Value {
        json!({
            "kind": kind, "name": "bem", "valueMinor": value,
            "acquiredOn": "2026-08-01", "book": book
        })
    }

    fn debt(balance: i64, book: &str) -> Value {
        json!({ "name": "d", "principalMinor": 999_999, "balanceMinor": balance, "book": book })
    }

    fn sale(kind: &str, amount: i64, on: &str) -> Value {
        json!({
            "kind": kind, "description": "x", "amountMinor": amount,
            "occurredOn": on, "book": "business"
        })
    }

    fn transactions() -> Vec<RawRow> {
        vec![
            raw(
                TRANSACTIONS,
                "T1",
                false,
                1,
                sale("income", 90_000, "2026-09-10"),
            ),
            raw(
                TRANSACTIONS,
                "T2",
                false,
                2,
                sale("expense", 25_000, "2026-09-12"),
            ),
            raw(
                TRANSACTIONS,
                "T3",
                false,
                3,
                sale("income", 10_000, "2026-08-10"),
            ),
        ]
    }

    /// Conta comercial: receita 900 e despesa 250 em setembro, receita 100 em agosto; bens
    /// 1.000 + 400 (investimento), 700 (estoque), 2.500 (equipamento); dividas 300 + 50.
    fn dataset() -> Dataset {
        let mut ds = Dataset::default();
        ds.apply(transactions());
        ds.apply([
            raw(
                ASSETS,
                "A1",
                false,
                4,
                asset("investment", 100_000, "business"),
            ),
            raw(
                ASSETS,
                "A2",
                false,
                5,
                asset("investment", 40_000, "business"),
            ),
            raw(
                ASSETS,
                "A3",
                false,
                6,
                asset("inventory", 70_000, "business"),
            ),
            raw(
                ASSETS,
                "A4",
                false,
                7,
                asset("equipment", 250_000, "business"),
            ),
            raw(DEBTS, "D1", false, 8, debt(30_000, "business")),
            raw(DEBTS, "D2", false, 9, debt(5_000, "business")),
        ]);
        ds
    }

    fn panel(ds: &Dataset) -> BusinessPanel {
        business_panel(&ds.for_book(Book::Business), "2026-09")
    }

    #[test]
    fn soma_por_tipo_e_capital_total() {
        let p = panel(&dataset());
        assert_eq!(p.investment_minor, 140_000);
        assert_eq!(p.inventory_minor, 70_000);
        assert_eq!(p.equipment_minor, 250_000);
        assert_eq!(p.capital_minor, 460_000);
        assert_eq!(p.debt_minor, 35_000);
    }

    #[test]
    fn resultado_do_mes_e_receita_menos_despesa() {
        let p = panel(&dataset());
        assert_eq!(p.income_minor, 90_000);
        assert_eq!(p.expense_minor, 25_000);
        assert_eq!(p.result_minor, 65_000);
        // Outro mes: so a receita de agosto.
        let aug = business_panel(&dataset().for_book(Book::Business), "2026-08");
        assert_eq!(
            (aug.income_minor, aug.expense_minor, aug.result_minor),
            (10_000, 0, 10_000)
        );
        // Capital e dividas nao dependem do mes.
        assert_eq!(aug.capital_minor, 460_000);
        assert_eq!(aug.debt_minor, 35_000);
    }

    #[test]
    fn resultado_negativo_fica_negativo() {
        let mut ds = Dataset::default();
        ds.apply([raw(
            TRANSACTIONS,
            "T1",
            false,
            1,
            sale("expense", 12_345, "2026-09-01"),
        )]);
        assert_eq!(panel(&ds).result_minor, -12_345);
    }

    #[test]
    fn linhas_apagadas_ficam_fora_da_soma() {
        let mut ds = dataset();
        ds.apply([
            raw(
                ASSETS,
                "A4",
                true,
                20,
                asset("equipment", 250_000, "business"),
            ),
            raw(
                ASSETS,
                "A9",
                true,
                21,
                asset("investment", 77_000, "business"),
            ),
            raw(DEBTS, "D1", true, 22, debt(30_000, "business")),
        ]);
        let p = panel(&ds);
        assert_eq!(p.equipment_minor, 0);
        assert_eq!(p.investment_minor, 140_000);
        assert_eq!(p.capital_minor, 210_000);
        assert_eq!(p.debt_minor, 5_000);
    }

    #[test]
    fn linha_fora_do_contrato_nao_soma() {
        let mut ds = dataset();
        let report = ds.apply([
            // Tipo de bem desconhecido e saldo que nao e inteiro: ignoradas e contadas.
            raw(ASSETS, "A7", false, 30, asset("imovel", 500_000, "business")),
            raw(
                DEBTS,
                "D7",
                false,
                31,
                json!({ "name": "d", "principalMinor": 1, "balanceMinor": "9", "book": "business" }),
            ),
        ]);
        assert_eq!(report.ignored_total, 2);
        let p = panel(&ds);
        assert_eq!(p.capital_minor, 460_000);
        assert_eq!(p.debt_minor, 35_000);
    }

    #[test]
    fn bens_e_dividas_nao_mudam_receita_nem_despesa() {
        let with = panel(&dataset());
        let mut only_tx = Dataset::default();
        only_tx.apply(transactions());
        let without = panel(&only_tx);
        assert_eq!(
            (with.income_minor, with.expense_minor, with.result_minor),
            (
                without.income_minor,
                without.expense_minor,
                without.result_minor
            )
        );
        assert_eq!((without.capital_minor, without.debt_minor), (0, 0));
    }

    #[test]
    fn conta_pessoal_nao_soma_bens_nem_dividas_do_negocio() {
        let mut ds = dataset();
        // Bem sem `book` (linha antiga) vale pessoal e nunca vira capital do negocio.
        ds.apply([
            raw(
                ASSETS,
                "A8",
                false,
                40,
                json!({
                    "kind": "equipment", "name": "n", "valueMinor": 9_999,
                    "acquiredOn": "2026-01-01"
                }),
            ),
            raw(DEBTS, "D8", false, 41, debt(8_888, "personal")),
        ]);
        let p = panel(&ds);
        assert_eq!(p.equipment_minor, 250_000);
        assert_eq!(p.debt_minor, 35_000);
        // A visao pessoal, mesmo passada por engano, da zero.
        let personal = business_panel(&ds.for_book(Book::Personal), "2026-09");
        assert_eq!(personal, BusinessPanel::default());
    }
}
