//! Faturas de cartao de credito calculadas dos lancamentos. Port de `cards/invoice.ts` do app
//! (`card.ts` ja vive no contrato: `contract::card_day` e `PaymentMethodFields::card`).
//!
//! Nada aqui e a fatura "do banco": e a soma do que foi lancado no cartao, e a aberta muda a
//! cada compra. Por isso a tela a rotula como estimativa.

use std::collections::BTreeMap;

use super::contract::Kind;
use super::dataset::{Dataset, PaymentMethod, Transaction};
use super::periods::{day_of_month_clamped, days_in_month, shift_month};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvoiceStatus {
    /// Ainda nao fechou em `today` (fecha hoje conta como aberta): o valor e estimativa.
    Open,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invoice {
    pub card_id: String,
    /// Mes em que a fatura fecha (`YYYY-MM`): e a identidade dela.
    pub month: String,
    /// Fechamento real, ja limitado ao ultimo dia do mes.
    pub closes_on: String,
    pub due_on: String,
    pub status: InvoiceStatus,
    /// Compras menos estornos (receita no cartao abate).
    pub total_minor: i64,
    /// Mais recentes primeiro (data, depois id, decrescentes).
    pub items: Vec<Transaction>,
}

/// `invoiceMonthOf` do app: compra ate o dia do fechamento (inclusive) cai na fatura que fecha
/// naquele mes; depois dele, na do mes seguinte. O fechamento e limitado ao ultimo dia do mes
/// (fechamento 31 em fevereiro fecha no 28 ou 29).
pub fn invoice_month_of(occurred_on: &str, closing_day: u8) -> String {
    let month = occurred_on.get(..7).unwrap_or(occurred_on);
    let day: u8 = occurred_on
        .get(8..10)
        .and_then(|d| d.parse().ok())
        .unwrap_or(0);
    if day <= closing_day.min(days_in_month(month)) {
        month.to_string()
    } else {
        shift_month(month, 1)
    }
}

/// `invoiceDates` do app: fechamento e vencimento da fatura que fecha em `month`. Vencimento
/// no mesmo dia do fechamento ou antes so pode ser o do mes seguinte (a fatura fecha e depois
/// vence); os dois dias sao limitados ao ultimo do mes.
pub fn invoice_dates(month: &str, closing_day: u8, due_day: u8) -> (String, String) {
    let due_month = if due_day <= closing_day {
        shift_month(month, 1)
    } else {
        month.to_string()
    };
    (
        day_of_month_clamped(month, i64::from(closing_day)),
        day_of_month_clamped(&due_month, i64::from(due_day)),
    )
}

/// `cardInvoices` do app: faturas de um cartao, mais recentes primeiro. A que contem `today`
/// sempre aparece (mesmo vazia); as outras so se tiveram compra. Cartao por completar nao tem
/// fatura. Entra o que conta nos totais, e so pelo `paymentMethodId` cru da linha, como o app
/// (sem seguir `mergedInto`: cartao nao e padrao fundido).
pub fn card_invoices(ds: &Dataset, method: &PaymentMethod, today: &str) -> Vec<Invoice> {
    let Some(card) = method.card.as_ref() else {
        return Vec::new();
    };
    let mut by_month: BTreeMap<String, Vec<&Transaction>> = BTreeMap::new();
    by_month.insert(invoice_month_of(today, card.closing_day), Vec::new());
    for row in ds.alive_transactions() {
        if row.payment_method_id.as_deref() != Some(method.id.as_str()) || !row.counts_in_totals() {
            continue;
        }
        by_month
            .entry(invoice_month_of(&row.occurred_on, card.closing_day))
            .or_default()
            .push(row);
    }
    by_month
        .into_iter()
        .rev()
        .map(|(month, mut items)| {
            let (closes_on, due_on) = invoice_dates(&month, card.closing_day, card.due_day);
            items.sort_by(|a, b| {
                b.occurred_on
                    .cmp(&a.occurred_on)
                    .then_with(|| b.id.cmp(&a.id))
            });
            let total_minor = items.iter().fold(0i64, |sum, t| match t.kind {
                Kind::Expense => sum.saturating_add(t.amount_minor),
                Kind::Income => sum.saturating_sub(t.amount_minor),
            });
            let status = if closes_on.as_str() >= today {
                InvoiceStatus::Open
            } else {
                InvoiceStatus::Closed
            };
            Invoice {
                card_id: method.id.clone(),
                month,
                closes_on,
                due_on,
                status,
                total_minor,
                items: items.into_iter().cloned().collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::dashboard::contract::{card_day, parse_payment_method};
    use crate::dashboard::dataset::{PAYMENT_METHODS, TRANSACTIONS};
    use crate::dashboard::dataset_of;

    fn merge(mut base: Value, over: Value) -> Value {
        if let (Some(b), Some(o)) = (base.as_object_mut(), over.as_object()) {
            for (k, v) in o {
                b.insert(k.clone(), v.clone());
            }
        }
        base
    }

    /// `card()` de `fixtures.fake.ts`: cartao completo, fecha 5 e vence 12.
    fn card(id: &str, over: Value) -> Value {
        merge(
            json!({
                "name": id, "icon": "credit-card", "color": "violet", "kind": "credit",
                "bank": id, "closingDay": 5, "dueDay": 12
            }),
            over,
        )
    }

    /// `legacyCard()`: o `credit` de antes, sem banco, fechamento nem vencimento.
    fn legacy_card(id: &str) -> Value {
        json!({ "name": id, "icon": "credit-card", "color": "violet", "kind": "credit" })
    }

    fn complete(data: Value) -> bool {
        parse_payment_method(&data.to_string())
            .map(|f| f.is_complete_card())
            .unwrap_or(false)
    }

    /// `isIncompleteCard` do app: credito sem os dados do cartao.
    fn incomplete(data: Value) -> bool {
        parse_payment_method(&data.to_string())
            .map(|f| f.is_credit() && !f.is_complete_card())
            .unwrap_or(false)
    }

    /// `card.test.ts`.
    mod card {
        use super::*;

        #[test]
        fn aceita_inteiros_de_1_a_31() {
            for day in [1, 15, 28, 31] {
                assert!(card_day(Some(&json!(day))).is_some(), "{day}");
            }
        }

        /// O `NaN` do app nao existe em JSON; os demais valores sao os mesmos.
        #[test]
        fn recusa_zero_32_decimal_texto_e_nulo() {
            for day in [
                json!(0),
                json!(32),
                json!(-1),
                json!(5.5),
                json!("5"),
                Value::Null,
            ] {
                assert_eq!(card_day(Some(&day)), None, "{day}");
            }
            assert_eq!(card_day(None), None);
        }

        #[test]
        fn credito_com_banco_e_os_dois_dias_e_completo() {
            assert!(complete(card("nu", json!({}))));
            assert!(!incomplete(card("nu", json!({}))));
        }

        #[test]
        fn credito_sem_algum_dos_tres_e_por_completar() {
            assert!(incomplete(legacy_card("velho")));
            assert!(incomplete(card("a", json!({ "bank": "  " }))));
            assert!(incomplete(card("b", json!({ "closingDay": null }))));
            assert!(incomplete(card("c", json!({ "dueDay": 99 }))));
        }

        #[test]
        fn outros_tipos_nunca_sao_cartao_por_completar() {
            let pix = card(
                "pix",
                json!({ "kind": "pix", "bank": null, "closingDay": null, "dueDay": null }),
            );
            assert!(!incomplete(pix.clone()));
            assert!(!complete(pix));
        }
    }

    /// `invoice.test.ts` (`invoiceMonthOf`).
    mod invoice_month_of {
        use super::*;

        #[test]
        fn compra_antes_do_fechamento_fica_na_fatura_que_fecha_no_mes() {
            assert_eq!(invoice_month_of("2026-10-03", 5), "2026-10");
        }

        #[test]
        fn compra_no_dia_do_fechamento_fica_na_fatura_que_fecha_naquele_dia() {
            assert_eq!(invoice_month_of("2026-10-05", 5), "2026-10");
        }

        #[test]
        fn compra_depois_do_fechamento_vai_para_a_fatura_do_mes_seguinte() {
            assert_eq!(invoice_month_of("2026-10-06", 5), "2026-11");
        }

        #[test]
        fn virada_de_ano_depois_do_fechamento_de_dezembro_cai_em_janeiro_do_ano_seguinte() {
            assert_eq!(invoice_month_of("2026-12-20", 10), "2027-01");
            assert_eq!(invoice_month_of("2026-12-10", 10), "2026-12");
            assert_eq!(invoice_month_of("2027-01-02", 10), "2027-01");
        }

        #[test]
        fn fechamento_no_31_em_mes_curto_vale_o_ultimo_dia() {
            // Fevereiro de 2027 tem 28 dias: o dia 28 ja e o fechamento.
            assert_eq!(invoice_month_of("2027-02-28", 31), "2027-02");
            assert_eq!(invoice_month_of("2027-03-01", 31), "2027-03");
            // Ano bissexto: o dia 29 fecha, e o 28 ainda e antes.
            assert_eq!(invoice_month_of("2028-02-28", 31), "2028-02");
            assert_eq!(invoice_month_of("2028-02-29", 31), "2028-02");
            assert_eq!(invoice_month_of("2028-03-01", 31), "2028-03");
        }

        #[test]
        fn fechamento_no_30_em_fevereiro_tambem_usa_o_ultimo_dia() {
            assert_eq!(invoice_month_of("2027-02-28", 30), "2027-02");
        }
    }

    /// `invoice.test.ts` (`invoiceDates`).
    mod invoice_dates {
        use super::*;

        fn dates(month: &str, closing: u8, due: u8) -> String {
            let (c, d) = invoice_dates(month, closing, due);
            format!("{c} {d}")
        }

        #[test]
        fn vencimento_depois_do_fechamento_fica_no_mesmo_mes() {
            assert_eq!(dates("2026-10", 5, 12), "2026-10-05 2026-10-12");
        }

        #[test]
        fn vencimento_menor_que_o_fechamento_cai_no_mes_seguinte() {
            assert_eq!(dates("2026-10", 25, 5), "2026-10-25 2026-11-05");
        }

        #[test]
        fn vencimento_no_mesmo_dia_do_fechamento_tambem_vai_para_o_mes_seguinte() {
            assert_eq!(dates("2026-10", 10, 10), "2026-10-10 2026-11-10");
        }

        #[test]
        fn virada_de_ano_fatura_de_dezembro_vence_em_janeiro() {
            assert_eq!(dates("2026-12", 25, 5), "2026-12-25 2027-01-05");
        }

        #[test]
        fn mes_curto_limita_fechamento_e_vencimento_ao_ultimo_dia() {
            assert_eq!(dates("2027-02", 31, 30), "2027-02-28 2027-03-30");
            assert_eq!(dates("2027-01", 20, 31), "2027-01-20 2027-01-31");
            // Vencimento 30 num mes de 30 dias, e 31 num mes de 30 dias.
            assert_eq!(dates("2026-04", 10, 31), "2026-04-10 2026-04-30");
        }
    }

    /// `invoice.test.ts` (`cardInvoices`).
    mod card_invoices {
        use super::*;

        /// `purchase()` de `fixtures.fake.ts`.
        fn purchase(method: Option<&str>, on: &str, amount: i64, over: Value) -> Value {
            merge(
                json!({
                    "kind": "expense", "description": "compra", "amountMinor": amount,
                    "occurredOn": on, "paymentMethodId": method
                }),
                over,
            )
        }

        fn invoices(
            methods: Vec<(&str, Value)>,
            purchases: Vec<(&str, bool, Value)>,
            card_id: &str,
            today: &str,
        ) -> Vec<Invoice> {
            let mut rows: Vec<(&str, &str, bool, Value)> = methods
                .into_iter()
                .map(|(id, data)| (PAYMENT_METHODS, id, false, data))
                .collect();
            rows.extend(
                purchases
                    .into_iter()
                    .map(|(id, deleted, data)| (TRANSACTIONS, id, deleted, data)),
            );
            let ds = dataset_of(rows);
            ds.payment_methods
                .get(card_id)
                .map(|m| card_invoices(&ds, m, today))
                .unwrap_or_default()
        }

        fn nu() -> (&'static str, Value) {
            ("nu", card("nu", json!({ "closingDay": 5, "dueDay": 12 })))
        }

        fn summary(list: &[Invoice]) -> Vec<(&str, InvoiceStatus, i64)> {
            list.iter()
                .map(|i| (i.month.as_str(), i.status, i.total_minor))
                .collect()
        }

        fn ids(invoice: &Invoice) -> Vec<&str> {
            invoice.items.iter().map(|t| t.id.as_str()).collect()
        }

        #[test]
        fn separa_a_fatura_aberta_das_anteriores() {
            let list = invoices(
                vec![nu()],
                vec![
                    (
                        "a",
                        false,
                        purchase(Some("nu"), "2026-09-20", 10_000, json!({})),
                    ),
                    (
                        "b",
                        false,
                        purchase(Some("nu"), "2026-10-02", 2_500, json!({})),
                    ),
                    (
                        "c",
                        false,
                        purchase(Some("nu"), "2026-10-06", 4_000, json!({})),
                    ),
                ],
                "nu",
                "2026-10-10",
            );
            // Hoje (dia 10, depois do fechamento do dia 5) esta na fatura que fecha em novembro.
            assert_eq!(
                summary(&list),
                [
                    ("2026-11", InvoiceStatus::Open, 4_000),
                    ("2026-10", InvoiceStatus::Closed, 12_500)
                ]
            );
            assert_eq!(
                (list[0].closes_on.as_str(), list[0].due_on.as_str()),
                ("2026-11-05", "2026-11-12")
            );
        }

        #[test]
        fn antes_do_fechamento_a_fatura_aberta_e_a_que_fecha_neste_mes() {
            let list = invoices(
                vec![nu()],
                vec![(
                    "a",
                    false,
                    purchase(Some("nu"), "2026-10-02", 1_000, json!({})),
                )],
                "nu",
                "2026-10-04",
            );
            assert_eq!(summary(&list)[0], ("2026-10", InvoiceStatus::Open, 1_000));
        }

        #[test]
        fn no_proprio_dia_do_fechamento_a_fatura_ainda_esta_aberta() {
            let list = invoices(vec![nu()], vec![], "nu", "2026-10-05");
            assert_eq!(
                (list[0].month.as_str(), list[0].status),
                ("2026-10", InvoiceStatus::Open)
            );
        }

        #[test]
        fn a_fatura_aberta_aparece_mesmo_sem_compra() {
            let list = invoices(vec![nu()], vec![], "nu", "2026-10-10");
            assert_eq!(list.len(), 1);
            assert_eq!(
                (list[0].status, list[0].total_minor),
                (InvoiceStatus::Open, 0)
            );
            assert!(list[0].items.is_empty());
        }

        #[test]
        fn so_soma_o_que_e_do_cartao_e_conta_nos_totais() {
            let list = invoices(
                vec![nu(), ("outro", card("outro", json!({})))],
                vec![
                    (
                        "a",
                        false,
                        purchase(Some("nu"), "2026-10-02", 1_000, json!({})),
                    ),
                    (
                        "outro",
                        false,
                        purchase(Some("outro"), "2026-10-02", 9_000, json!({})),
                    ),
                    (
                        "transf",
                        false,
                        purchase(
                            Some("nu"),
                            "2026-10-02",
                            7_000,
                            json!({ "reportState": "transfer" }),
                        ),
                    ),
                    (
                        "oculta",
                        false,
                        purchase(
                            Some("nu"),
                            "2026-10-02",
                            6_000,
                            json!({ "reportState": "hidden" }),
                        ),
                    ),
                    (
                        "apagada",
                        true,
                        purchase(Some("nu"), "2026-10-02", 5_000, json!({})),
                    ),
                    ("sem", false, purchase(None, "2026-10-02", 4_000, json!({}))),
                ],
                "nu",
                "2026-10-04",
            );
            assert_eq!(ids(&list[0]), ["a"]);
            assert_eq!(list[0].total_minor, 1_000);
        }

        #[test]
        fn estorno_no_cartao_abate_a_fatura() {
            let list = invoices(
                vec![nu()],
                vec![
                    (
                        "a",
                        false,
                        purchase(Some("nu"), "2026-10-02", 10_000, json!({})),
                    ),
                    (
                        "estorno",
                        false,
                        purchase(Some("nu"), "2026-10-03", 2_500, json!({ "kind": "income" })),
                    ),
                ],
                "nu",
                "2026-10-04",
            );
            assert_eq!(list[0].total_minor, 7_500);
        }

        #[test]
        fn virada_de_ano_compra_de_dezembro_depois_do_fechamento_entra_na_fatura_de_janeiro() {
            let list = invoices(
                vec![nu()],
                vec![
                    (
                        "a",
                        false,
                        purchase(Some("nu"), "2026-12-28", 3_000, json!({})),
                    ),
                    (
                        "b",
                        false,
                        purchase(Some("nu"), "2027-01-03", 1_000, json!({})),
                    ),
                ],
                "nu",
                "2027-01-04",
            );
            assert_eq!(list.len(), 1);
            assert_eq!(
                (list[0].month.as_str(), list[0].total_minor),
                ("2027-01", 4_000)
            );
            assert_eq!(list[0].due_on, "2027-01-12");
        }

        #[test]
        fn lista_os_lancamentos_do_mais_recente_para_o_mais_antigo() {
            let list = invoices(
                vec![nu()],
                vec![
                    (
                        "a",
                        false,
                        purchase(Some("nu"), "2026-10-01", 1_000, json!({})),
                    ),
                    (
                        "b",
                        false,
                        purchase(Some("nu"), "2026-10-03", 1_000, json!({})),
                    ),
                ],
                "nu",
                "2026-10-04",
            );
            assert_eq!(ids(&list[0]), ["b", "a"]);
        }

        #[test]
        fn cartao_por_completar_nao_tem_fatura() {
            let list = invoices(
                vec![("velho", legacy_card("velho"))],
                vec![(
                    "a",
                    false,
                    purchase(Some("velho"), "2026-10-02", 1_000, json!({})),
                )],
                "velho",
                "2026-10-04",
            );
            assert!(list.is_empty());
        }

        #[test]
        fn dia_invalido_vindo_de_outra_versao_degrada_para_cartao_por_completar() {
            let strange = card("estranho", json!({ "closingDay": 45 }));
            let list = invoices(
                vec![("estranho", strange)],
                vec![],
                "estranho",
                "2026-10-04",
            );
            assert!(list.is_empty());
        }

        /// Fatura do `PMP` da fixture Grafite (spec 3.2): fecha 28, vence 8 do mes seguinte.
        #[test]
        fn fatura_de_novembro_do_mercado_pago_vence_em_8_de_dezembro() {
            let pmp = card(
                "PMP",
                json!({ "bank": "Mercado Pago", "closingDay": 28, "dueDay": 8 }),
            );
            let list = invoices(
                vec![("PMP", pmp)],
                vec![
                    (
                        "1103-1-COMPRAS",
                        false,
                        purchase(Some("PMP"), "2026-11-03", 29_990, json!({})),
                    ),
                    (
                        "1114-1-COMPRAS",
                        false,
                        purchase(Some("PMP"), "2026-11-14", 21_525, json!({})),
                    ),
                ],
                "PMP",
                "2026-11-19",
            );
            assert_eq!(summary(&list), [("2026-11", InvoiceStatus::Open, 51_515)]);
            assert_eq!(
                (list[0].closes_on.as_str(), list[0].due_on.as_str()),
                ("2026-11-28", "2026-12-08")
            );
        }
    }
}
