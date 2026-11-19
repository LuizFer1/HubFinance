//! Projecao de recorrencia: valor vigente (reajuste, 1a parcela), estimativa de serie "valor
//! varia", ocorrencias por vir e o "comprometido" de um mes. Port de `adjustments.ts`,
//! `estimate.ts`, `upcoming.ts` e `free-money.ts` do app: as telas do hub e do celular tem de
//! prometer o mesmo valor para a mesma ocorrencia.
//!
//! Calculado, nunca gravado (o hub nunca edita dado). Ocorrencia que o hub ja conhece pelo id
//! estavel (`Dataset::knows_transaction`) fica de fora da projecao, entao nada conta duas vezes.

use super::contract::{Kind, RecurrenceRule};
use super::dataset::{Dataset, RecurrenceAdjustment, Transaction};
use super::ids::stable_entity_id;
use super::periods::{days_in_month, last_day_of_month, shift_day, shift_month};
use super::reserves::div_round;
use super::schedule::{each_period, occurrence_key, occurrence_on, period_of, period_of_key};

/// `ESTIMATE_WINDOW` do app: ocorrencias, nao meses (numa bimestral seis meses seriam tres
/// pontos).
pub const ESTIMATE_WINDOW: usize = 6;

/// Uma ocorrencia ainda nao gravada (`UpcomingOccurrence` do app).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Occurrence {
    pub recurrence_id: String,
    /// Competencia: `YYYY-MM`, ou a data em semanal/quinzenal.
    pub period: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    pub kind: Kind,
    pub description: Option<String>,
    pub amount_minor: i64,
    /// A da serie (o tile de "Proximas saidas" segue a serie, spec 3.1).
    pub category_id: Option<String>,
    pub payment_method_id: Option<String>,
    /// Serie "valor varia": `amount_minor` e a estimativa de hoje, nao um valor certo.
    pub estimated: bool,
}

/// `amountFor` do app: valor vigente da serie na competencia. O reajuste vivo de maior
/// `fromPeriod <= period` (comparacao de texto, como o app) vence; sem nenhum, a 1a parcela
/// usa `firstAmountMinor` (a sobra de centavos) e as demais o valor-base.
///
/// Filtra serie e `deleted` aqui dentro: quem chama passa a tabela inteira, e um reajuste
/// desfeito que ainda valesse faria a projecao prometer um valor que o usuario apagou.
/// Dois reajustes vivos na mesma competencia acontecem: o id estavel v1 colide entre series
/// (um aparelho novo grava a chave em v2, um velho a regrava em v1) e dois aparelhos podem
/// reajustar a mesma competencia offline. Desempate pela regra R, igual a do app e a de
/// `budgets::current_budgets`: `updated_at` maior (HLC de largura fixa, a ordem de texto e a
/// semantica; ausente perde de qualquer presente), depois `id` maior. `updated_at` antes do id
/// porque a ultima edicao e a intencao do usuario; o id so desempata para o numero nao oscilar
/// com a ordem do mapa nem divergir do celular.
pub fn amount_for<'a>(
    recurrence_id: &str,
    rule: &RecurrenceRule,
    adjustments: impl IntoIterator<Item = &'a RecurrenceAdjustment>,
    period: &str,
) -> i64 {
    let best = adjustments
        .into_iter()
        .filter(|a| !a.deleted && a.recurrence_id == recurrence_id)
        .filter(|a| a.from_period.as_str() <= period)
        .max_by(|a, b| {
            a.from_period
                .cmp(&b.from_period)
                .then_with(|| a.updated_at.as_deref().cmp(&b.updated_at.as_deref()))
                .then_with(|| a.id.cmp(&b.id))
        });
    if let Some(adjustment) = best {
        return adjustment.amount_minor;
    }
    if period_of(rule.frequency, &rule.start_on) == period {
        rule.first_amount_minor.unwrap_or(rule.amount_minor)
    } else {
        rule.amount_minor
    }
}

/// `estimateFor` do app: media (`div_round`) das `ESTIMATE_WINDOW` ocorrencias **confirmadas**
/// mais recentes da serie **antes** de `period`, pela competencia da `occurrenceKey`; sem
/// nenhuma, `base_minor` (o palpite da serie).
///
/// So confirmadas: estimativa que entrasse na media se realimentaria. So anteriores: a
/// estimativa de marco nao pode depender de abril. Competencias iguais (duas linhas com a
/// mesma chave) desempatam pelo `id`, porque a ordem do `HashMap` muda a cada execucao.
pub fn estimate_for<'a>(
    recurrence_id: &str,
    base_minor: i64,
    transactions: impl IntoIterator<Item = &'a Transaction>,
    period: &str,
) -> i64 {
    let mut confirmed: Vec<(&str, &str, i64)> = transactions
        .into_iter()
        .filter(|t| !t.deleted && !t.estimated)
        .filter(|t| t.recurrence_id.as_deref() == Some(recurrence_id))
        .filter_map(|t| {
            let own = period_of_key(t.occurrence_key.as_deref()?);
            (own < period).then_some((own, t.id.as_str(), t.amount_minor))
        })
        .collect();
    if confirmed.is_empty() {
        return base_minor;
    }
    confirmed.sort_by(|a, b| b.0.cmp(a.0).then_with(|| a.1.cmp(b.1)));
    let recent = &confirmed[..confirmed.len().min(ESTIMATE_WINDOW)];
    let sum = recent
        .iter()
        .fold(0i64, |acc, (_, _, amount)| acc.saturating_add(*amount));
    let count = i64::try_from(recent.len()).unwrap_or(i64::MAX);
    div_round(sum, count)
}

/// `projectRecurrences` do app: ocorrencias ainda nao gravadas de series vivas e ativas com
/// data em (`after`, `limit`], ordenadas por data e, na mesma data, pelo id da serie (o app
/// herda a ordem de insercao; aqui o mapa nao tem ordem e a lista nao pode piscar).
///
/// Serie sem `rule` (app antigo, campo de tipo errado, frequencia de versao futura) nao
/// projeta. Ocorrencia que o hub ja conhece pelo id estavel (viva, apagada ou ignorada) fica
/// de fora: e a mesma regra de presenca do app, e uma ocorrencia com data editada continua
/// deduplicada.
pub fn project(ds: &Dataset, after: &str, limit: &str) -> Vec<Occurrence> {
    let mut found = Vec::new();
    for series in ds.recurrences.values() {
        if series.deleted {
            continue;
        }
        let Some(rule) = series.rule.as_ref().filter(|r| r.active) else {
            continue;
        };
        for period in each_period(&rule.start_on, limit, rule.frequency) {
            let Some(date) = occurrence_on(&period, rule.schedule) else {
                continue;
            };
            if date.as_str() <= after || date.as_str() > limit {
                continue;
            }
            if rule
                .end_on
                .as_deref()
                .is_some_and(|end| date.as_str() > end)
            {
                continue;
            }
            if ds.knows_transaction(&stable_entity_id(&occurrence_key(&series.id, &period))) {
                continue;
            }
            let amount_minor = if rule.variable {
                estimate_for(
                    &series.id,
                    rule.amount_minor,
                    ds.transactions.values(),
                    &period,
                )
            } else {
                amount_for(
                    &series.id,
                    rule,
                    ds.recurrence_adjustments.values(),
                    &period,
                )
            };
            found.push(Occurrence {
                recurrence_id: series.id.clone(),
                period,
                date,
                kind: rule.kind,
                description: series.description.clone(),
                amount_minor,
                category_id: series.category_id.clone(),
                payment_method_id: series.payment_method_id.clone(),
                estimated: rule.variable,
            });
        }
    }
    found.sort_by(|a, b| {
        a.date
            .cmp(&b.date)
            .then_with(|| a.recurrence_id.cmp(&b.recurrence_id))
    });
    found
}

/// "Proximas saidas" (`upcomingRecurrences` do app): janela (`today`, `today + days`]. So o
/// futuro: ocorrencia vencida e ainda nao sincronizada nao aparece aqui, so na previsao do mes
/// (`project_month`).
pub fn upcoming(ds: &Dataset, today: &str, days: i64) -> Vec<Occurrence> {
    project(ds, today, &shift_day(today, days))
}

/// Tudo o que a regra ainda gera com data em `month` (`YYYY-MM`), vencido ou nao: a janela
/// (ultimo dia do mes anterior, ultimo dia do mes]. E a fonte do "por vir" da previsao
/// (spec 5.6) e do comprometido (`recurring_month`). Mes invalido -> vazio.
pub fn project_month(ds: &Dataset, month: &str) -> Vec<Occurrence> {
    if days_in_month(month) == 0 {
        return Vec::new();
    }
    let before = last_day_of_month(&shift_month(month, -1));
    project(ds, &before, &last_day_of_month(month))
}

/// Entradas e saidas recorrentes de um mes (`FreeMoney` do app).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RecurringMonth {
    pub month: String,
    pub income_minor: i64,
    /// O dinheiro ja comprometido.
    pub expense_minor: i64,
    /// `income - expense`; negativo quando as saidas passam das entradas.
    pub free_minor: i64,
    /// Alguma parcela vem de serie "valor varia" ou de lancamento estimado: a tela poe "~".
    pub estimated: bool,
}

/// `recurringMonth` do app: o que a regra ainda vai gerar no mes mais as ocorrencias ja
/// gravadas com data nele (vivas, com `recurrenceId`, que contam nos totais). A projecao pula
/// a chave que existe, entao nada conta duas vezes; em mes passado so sobra a ocorrencia
/// vencida sem linha, exatamente a que o celular grava no proximo boot com o mesmo valor.
/// Lancamento avulso datado para frente nao entra: a pergunta e o que a recorrencia
/// comprometeu.
pub fn recurring_month(ds: &Dataset, month: &str) -> RecurringMonth {
    let mut out = RecurringMonth {
        month: month.to_string(),
        ..RecurringMonth::default()
    };
    let mut add = |kind: Kind, amount: i64, estimated: bool| {
        match kind {
            Kind::Income => out.income_minor = out.income_minor.saturating_add(amount),
            Kind::Expense => out.expense_minor = out.expense_minor.saturating_add(amount),
        }
        out.estimated |= estimated;
    };
    for item in project_month(ds, month) {
        add(item.kind, item.amount_minor, item.estimated);
    }
    for row in ds.alive_transactions() {
        if row.recurrence_id.is_none() || !row.counts_in_totals() {
            continue;
        }
        if row.occurred_on.get(..7) != Some(month) {
            continue;
        }
        add(row.kind, row.amount_minor, row.estimated);
    }
    out.free_minor = out.income_minor.saturating_sub(out.expense_minor);
    out
}

/// "25% da renda fixa" (`committedPercent` do app, spec 6.2.8): quanto das entradas recorrentes as
/// saidas recorrentes ja levam. Sem teto (150% e informacao; quem trava em 100% e a barra).
/// `None` sem entrada recorrente: nao ha renda fixa para ser a regua.
pub fn committed_percent(m: &RecurringMonth) -> Option<i64> {
    (m.income_minor > 0).then(|| {
        super::aggregate::js_round(m.expense_minor as f64 / m.income_minor as f64 * 100.0) as i64
    })
}

/// `nextMonthFreeMoney` do app: o livre do mes seguinte ao de `today`.
pub fn next_month_free(ds: &Dataset, today: &str) -> RecurringMonth {
    let month = today.get(..7).unwrap_or(today);
    recurring_month(ds, &shift_month(month, 1))
}

#[cfg(test)]
mod grafite_tests {
    use super::*;
    use crate::dashboard::{GRAFITE_TODAY, fixtures_grafite};

    #[test]
    fn livre_em_dezembro() {
        // 3.2: 350000 - 87600 = 262400 -> "R$ 2.624", sem "~" (nenhuma serie variavel).
        let dec = next_month_free(&fixtures_grafite(), GRAFITE_TODAY);
        assert_eq!(dec.month, "2026-12");
        assert_eq!(
            (dec.income_minor, dec.expense_minor, dec.free_minor),
            (350_000, 87_600, 262_400)
        );
        assert!(!dec.estimated);
    }

    #[test]
    fn comprometimento() {
        // 3.2: js_round(87600 / 350000 x 100) = 25.
        let dec = next_month_free(&fixtures_grafite(), GRAFITE_TODAY);
        assert_eq!(committed_percent(&dec), Some(25));
    }

    #[test]
    fn comprometimento_sem_receita_e_sem_teto() {
        let month = |income_minor, expense_minor| RecurringMonth {
            month: "2026-12".into(),
            income_minor,
            expense_minor,
            free_minor: income_minor - expense_minor,
            estimated: false,
        };
        assert_eq!(committed_percent(&month(0, 1_000)), None);
        assert_eq!(committed_percent(&month(1_000, 1_500)), Some(150));
    }

    #[test]
    fn proximas_saidas_da_fixture() {
        // So a Cadeira (21/11, "em 2 dias") nos proximos 30 dias, antes das de dezembro.
        let next = upcoming(&fixtures_grafite(), GRAFITE_TODAY, 30);
        let first = next.first().expect("cadeira");
        assert_eq!(
            (
                first.recurrence_id.as_str(),
                first.date.as_str(),
                first.amount_minor
            ),
            ("RCAD", "2026-11-21", 5_052)
        );
        assert!(next.iter().all(|o| o.date.as_str() > GRAFITE_TODAY));
        // "Ver todas as 9" do app = 1 fatura + estas 8 (receita inclusive) em (19/11, 19/12].
        assert_eq!(next.len(), 8);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::dashboard::dataset::{RECURRENCE_ADJUSTMENTS, RECURRENCES, TRANSACTIONS};
    use crate::dashboard::dataset_of;

    type Row<'a> = (&'a str, &'a str, bool, Value);

    fn merge(mut base: Value, over: Value) -> Value {
        if let (Some(b), Some(o)) = (base.as_object_mut(), over.as_object()) {
            for (k, v) in o {
                b.insert(k.clone(), v.clone());
            }
        }
        base
    }

    fn series_json(base: Value, over: Value) -> Value {
        merge(base, over)
    }

    fn adjustment<'a>(id: &'a str, recurrence: &str, from: &str, amount: i64) -> Row<'a> {
        (
            RECURRENCE_ADJUSTMENTS,
            id,
            false,
            json!({ "recurrenceId": recurrence, "fromPeriod": from, "amountMinor": amount }),
        )
    }

    /// Ocorrencia gravada: id estavel da chave, como o app materializa.
    fn occurrence(
        recurrence: &str,
        kind: &str,
        period: &str,
        occurred_on: &str,
        amount: i64,
        over: Value,
    ) -> (String, Value) {
        let key = occurrence_key(recurrence, period);
        let data = merge(
            json!({
                "kind": kind,
                "description": "ocorrencia",
                "amountMinor": amount,
                "occurredOn": occurred_on,
                "recurrenceId": recurrence,
                "occurrenceKey": key,
            }),
            over,
        );
        (stable_entity_id(&key), data)
    }

    fn rule_of<'a>(ds: &'a Dataset, id: &str) -> &'a RecurrenceRule {
        ds.recurrences
            .get(id)
            .and_then(|r| r.rule.as_ref())
            .expect("serie com regra")
    }

    fn dates_and_amounts(found: &[Occurrence]) -> Vec<(&str, i64)> {
        found
            .iter()
            .map(|o| (o.date.as_str(), o.amount_minor))
            .collect()
    }

    /// `adjustments.test.ts` (`amountFor`).
    mod amount_for {
        use super::*;

        fn serie() -> Value {
            // O app testa sem `startOn`; aqui a regra exige um, entao ele fica longe das
            // competencias pedidas (a 1a parcela nao interfere).
            json!({
                "frequency": "monthly", "kind": "expense", "amountMinor": 300_000,
                "scheduleType": "dayOfMonth", "scheduleN": 5, "startOn": "2020-01-05"
            })
        }

        fn amount(extra: Vec<Row>, period: &str) -> i64 {
            amount_with(serie(), extra, period)
        }

        fn amount_with(series: Value, extra: Vec<Row>, period: &str) -> i64 {
            let mut rows = vec![(RECURRENCES, "SERIE-1", false, series)];
            rows.extend(extra);
            let ds = dataset_of(rows);
            amount_for(
                "SERIE-1",
                rule_of(&ds, "SERIE-1"),
                ds.recurrence_adjustments.values(),
                period,
            )
        }

        #[test]
        fn sem_reajuste_vale_o_valor_base() {
            assert_eq!(amount(vec![], "2027-06"), 300_000);
        }

        #[test]
        fn vale_a_partir_da_competencia_do_reajuste_inclusive() {
            let lista = || vec![adjustment("A1", "SERIE-1", "2027-01", 350_000)];
            assert_eq!(amount(lista(), "2026-12"), 300_000);
            assert_eq!(amount(lista(), "2027-01"), 350_000);
            assert_eq!(amount(lista(), "2027-08"), 350_000);
        }

        #[test]
        fn com_varios_vale_o_mais_recente_ate_a_competencia_em_qualquer_ordem() {
            let lista = || {
                vec![
                    adjustment("A2", "SERIE-1", "2028-01", 400_000),
                    adjustment("A1", "SERIE-1", "2027-01", 350_000),
                ]
            };
            assert_eq!(amount(lista(), "2027-06"), 350_000);
            assert_eq!(amount(lista(), "2028-01"), 400_000);
        }

        #[test]
        fn reajuste_apagado_nao_vale() {
            let mut row = adjustment("A1", "SERIE-1", "2027-01", 350_000);
            row.2 = true;
            assert_eq!(amount(vec![row], "2027-06"), 300_000);
        }

        #[test]
        fn reajuste_de_outra_serie_nao_interfere() {
            let row = adjustment("A1", "OUTRA", "2027-01", 350_000);
            assert_eq!(amount(vec![row], "2027-06"), 300_000);
        }

        /// `describe("1ª parcela com a sobra")`.
        mod primeira_parcela_com_a_sobra {
            use super::*;

            fn parcelada(first: Value) -> Value {
                json!({
                    "frequency": "monthly", "kind": "expense", "amountMinor": 3_333,
                    "firstAmountMinor": first, "scheduleType": "dayOfMonth", "scheduleN": 15,
                    "startOn": "2026-11-15"
                })
            }

            #[test]
            fn vale_so_na_competencia_inicial() {
                assert_eq!(
                    amount_with(parcelada(json!(3_334)), vec![], "2026-11"),
                    3_334
                );
                assert_eq!(
                    amount_with(parcelada(json!(3_334)), vec![], "2026-12"),
                    3_333
                );
            }

            #[test]
            fn reajuste_que_cobre_a_competencia_inicial_vence() {
                let row = adjustment("A1", "SERIE-1", "2026-11", 5_000);
                assert_eq!(
                    amount_with(parcelada(json!(3_334)), vec![row], "2026-11"),
                    5_000
                );
            }

            #[test]
            fn valor_invalido_vindo_do_sync_degrada_para_o_valor_base() {
                for bad in [json!(0), json!(1.5), Value::Null] {
                    assert_eq!(amount_with(parcelada(bad), vec![], "2026-11"), 3_333);
                }
            }
        }

        #[test]
        fn dois_reajustes_na_mesma_competencia_desempatam_pelo_id() {
            for _ in 0..8 {
                let lista = vec![
                    adjustment("A1", "SERIE-1", "2027-01", 350_000),
                    adjustment("A9", "SERIE-1", "2027-01", 360_000),
                ];
                assert_eq!(amount(lista, "2027-03"), 360_000);
            }
        }

        /// Regra R (spec 2026-11-19-id-estavel-colisao, secoes 4.3 e 8): entre reajustes vivos da
        /// mesma serie e competencia vale o de `updatedAt` maior; empate, o de id maior; sem
        /// `updatedAt`, perde de qualquer um com. Os vetores sao os mesmos do app (P1-2): mudar
        /// um aqui sem mudar la faz o hub e o celular mostrarem valores diferentes.
        mod regra_r {
            use super::*;

            const SERIES: &str = "01JZ0000000000000000000SR1";
            const ID_A: &str = "01JZ000000000000000000ADJA";
            const ID_B: &str = "01JZ000000000000000000ADJB";
            const H1: &str = "1793000000000-0000-01JZ0000000000000000000DV1";
            const H2: &str = "1793000000000-0001-01JZ0000000000000000000DV2";

            fn row<'a>(
                id: &'a str,
                updated_at: Option<&str>,
                amount: i64,
                deleted: bool,
            ) -> Row<'a> {
                let mut data = json!({
                    "recurrenceId": SERIES, "fromPeriod": "2027-01", "amountMinor": amount
                });
                if let Some(version) = updated_at {
                    data["updatedAt"] = json!(version);
                }
                (RECURRENCE_ADJUSTMENTS, id, deleted, data)
            }

            /// Valor em 2027-03 com as linhas nas duas ordens; as duas tem de dar o mesmo.
            fn winner(a: Row, b: Row) -> i64 {
                let ds = dataset_of(vec![(RECURRENCES, SERIES, false, serie()), a, b]);
                let mut rows: Vec<&RecurrenceAdjustment> =
                    ds.recurrence_adjustments.values().collect();
                rows.sort_by(|x, y| x.id.cmp(&y.id));
                let rule = rule_of(&ds, SERIES);
                let forward = amount_for(SERIES, rule, rows.iter().copied(), "2027-03");
                let backward = amount_for(SERIES, rule, rows.iter().rev().copied(), "2027-03");
                assert_eq!(forward, backward, "a ordem mudou o vencedor");
                forward
            }

            #[test]
            fn r1_updated_at_maior_vence_mesmo_com_id_menor() {
                let a = row(ID_A, Some(H2), 350_000, false);
                let b = row(ID_B, Some(H1), 360_000, false);
                assert_eq!(winner(a, b), 350_000);
            }

            #[test]
            fn r2_empate_de_updated_at_vence_o_id_maior() {
                let a = row(ID_A, Some(H1), 350_000, false);
                let b = row(ID_B, Some(H1), 360_000, false);
                assert_eq!(winner(a, b), 360_000);
            }

            #[test]
            fn r3_sem_updated_at_perde_mesmo_com_id_maior() {
                let a = row(ID_A, Some(H1), 350_000, false);
                let b = row(ID_B, None, 360_000, false);
                assert_eq!(winner(a, b), 350_000);
            }

            #[test]
            fn r4_apagado_nao_concorre_mesmo_mais_novo() {
                let a = row(ID_A, Some(H2), 350_000, true);
                let b = row(ID_B, Some(H1), 360_000, false);
                assert_eq!(winner(a, b), 360_000);
            }
        }
    }

    /// `estimate.test.ts`.
    mod estimate_for {
        use super::*;

        fn luz(over: Value) -> Value {
            merge(
                json!({
                    "kind": "expense", "description": "Conta de luz", "amountMinor": 20_000,
                    "frequency": "monthly", "scheduleType": "dayOfMonth", "scheduleN": 10,
                    "startOn": "2026-01-10", "endOn": null, "active": true, "variable": true
                }),
                over,
            )
        }

        /// O app monta `occurredOn` como `${period}-10`; numa competencia semanal isso da
        /// `2026-03-04-10`, que o contrato do hub recusa (a linha iria para ignoradas). Aqui a
        /// data e a propria competencia quando ela ja e data; o vetor nao muda.
        fn occ(series: &str, period: &str, amount: i64, over: Value) -> (String, Value) {
            let on = if period.len() == 10 {
                period.to_string()
            } else {
                format!("{period}-10")
            };
            occurrence(series, "expense", period, &on, amount, over)
        }

        fn estimate(series: &str, body: Value, occs: Vec<(String, Value, bool)>, p: &str) -> i64 {
            let mut rows: Vec<Row> = vec![(RECURRENCES, series, false, body)];
            for (id, data, deleted) in &occs {
                rows.push((TRANSACTIONS, id.as_str(), *deleted, data.clone()));
            }
            let ds = dataset_of(rows);
            let rule = rule_of(&ds, series);
            estimate_for(series, rule.amount_minor, ds.transactions.values(), p)
        }

        fn alive(o: (String, Value)) -> (String, Value, bool) {
            (o.0, o.1, false)
        }

        #[test]
        fn sem_nenhuma_confirmada_usa_o_palpite_da_serie() {
            assert_eq!(
                estimate("SERIE-LUZ", luz(json!({})), vec![], "2026-03"),
                20_000
            );
        }

        #[test]
        fn e_a_media_das_confirmadas_anteriores_arredondada_para_centavo() {
            let rows = vec![
                alive(occ("SERIE-LUZ", "2026-01", 10_000, json!({}))),
                alive(occ("SERIE-LUZ", "2026-02", 10_001, json!({}))),
            ];
            assert_eq!(
                estimate("SERIE-LUZ", luz(json!({})), rows, "2026-03"),
                10_001
            );
        }

        #[test]
        fn usa_so_as_6_competencias_mais_recentes() {
            let mut rows = vec![
                alive(occ("SERIE-LUZ", "2026-01", 90_000, json!({}))),
                alive(occ("SERIE-LUZ", "2026-02", 90_000, json!({}))),
            ];
            for m in ["03", "04", "05", "06", "07", "08"] {
                rows.push(alive(occ(
                    "SERIE-LUZ",
                    &format!("2026-{m}"),
                    30_000,
                    json!({}),
                )));
            }
            assert_eq!(
                estimate("SERIE-LUZ", luz(json!({})), rows, "2026-09"),
                30_000
            );
        }

        #[test]
        fn ignora_estimadas_apagadas_de_outra_serie_e_competencias_a_partir_da_pedida() {
            let (id3, d3) = occ("SERIE-LUZ", "2026-03", 99_000, json!({}));
            let rows = vec![
                alive(occ("SERIE-LUZ", "2026-01", 10_000, json!({}))),
                alive(occ(
                    "SERIE-LUZ",
                    "2026-02",
                    99_000,
                    json!({ "estimated": true }),
                )),
                (id3, d3, true),
                alive(occ("SERIE-AGUA", "2026-02", 99_000, json!({}))),
                alive(occ("SERIE-LUZ", "2026-04", 99_000, json!({}))),
                alive(occ("SERIE-LUZ", "2026-05", 99_000, json!({}))),
            ];
            assert_eq!(
                estimate("SERIE-LUZ", luz(json!({})), rows, "2026-04"),
                10_000
            );
        }

        #[test]
        fn conta_como_confirmada_a_ocorrencia_sem_a_coluna_lancada_quando_a_serie_era_fixa() {
            let rows = vec![
                alive(occ("SERIE-LUZ", "2026-01", 15_000, json!({}))),
                alive(occ(
                    "SERIE-LUZ",
                    "2026-02",
                    25_000,
                    json!({ "estimated": false }),
                )),
            ];
            assert_eq!(
                estimate("SERIE-LUZ", luz(json!({})), rows, "2026-03"),
                20_000
            );
        }

        #[test]
        fn serie_bimestral_conta_ocorrencias_nao_meses() {
            let mut rows: Vec<_> = ["01", "03", "05", "07", "09", "11"]
                .iter()
                .map(|m| alive(occ("SERIE-LUZ", &format!("2026-{m}"), 6_000, json!({}))))
                .collect();
            rows.push(alive(occ("SERIE-LUZ", "2025-11", 60_000, json!({}))));
            let bi = luz(json!({ "frequency": "bimonthly" }));
            assert_eq!(estimate("SERIE-LUZ", bi, rows, "2027-01"), 6_000);
        }

        /// `describe("série semanal")`.
        #[test]
        fn serie_semanal_compara_competencias_em_data_e_usa_as_6_mais_recentes_antes_da_atual() {
            let sem = || luz(json!({ "frequency": "weekly" }));
            let days = [
                "2026-03-04",
                "2026-03-11",
                "2026-03-18",
                "2026-03-25",
                "2026-04-01",
                "2026-04-08",
            ];
            let rows = || {
                let mut rows: Vec<_> = days
                    .iter()
                    .zip(1..)
                    .map(|(d, i)| alive(occ("SERIE-LUZ", d, 1_000 * i, json!({}))))
                    .collect();
                rows.push(alive(occ("SERIE-LUZ", "2026-02-25", 90_000, json!({}))));
                rows
            };
            // 1000..6000 -> media 3500; a de fevereiro fica fora da janela de 6.
            assert_eq!(estimate("SERIE-LUZ", sem(), rows(), "2026-04-15"), 3_500);
            // As posteriores a competencia nao entram.
            assert_eq!(
                estimate("SERIE-LUZ", sem(), rows(), "2026-03-11"),
                (90_000 + 1_000) / 2
            );
        }
    }

    /// `upcoming.test.ts` (`upcomingRecurrences` e os semanais que nao sao `nextOccurrences`).
    mod upcoming {
        use super::*;

        const SERIES: &str = "01J9F3K2M7QX8YB4TVWZ0DCEHS";

        fn series(over: Value) -> Value {
            series_json(
                json!({
                    "kind": "expense", "description": "Internet", "amountMinor": 12_000,
                    "frequency": "monthly", "scheduleType": "dayOfMonth", "scheduleN": 30,
                    "startOn": "2026-06-30", "endOn": null, "active": true
                }),
                over,
            )
        }

        fn with(series: Value) -> Dataset {
            dataset_of(vec![(RECURRENCES, SERIES, false, series)])
        }

        #[test]
        fn projeta_o_valor_reajustado_a_partir_da_competencia_do_reajuste() {
            let ds = dataset_of(vec![
                (RECURRENCES, SERIES, false, series(json!({}))),
                adjustment("AJ", SERIES, "2026-09", 13_000),
            ]);
            let found = upcoming(&ds, "2026-08-10", 60);
            assert_eq!(
                dates_and_amounts(&found),
                [("2026-08-30", 12_000), ("2026-09-30", 13_000)]
            );
        }

        #[test]
        fn lista_as_ocorrencias_dos_proximos_dias_depois_de_hoje() {
            let found = upcoming(&with(series(json!({}))), "2026-09-24", 30);
            assert_eq!(dates_and_amounts(&found), [("2026-09-30", 12_000)]);
            assert_eq!(found[0].description.as_deref(), Some("Internet"));
        }

        #[test]
        fn janela_maior_pega_a_competencia_seguinte() {
            let found = upcoming(&with(series(json!({}))), "2026-09-24", 40);
            let dates: Vec<_> = found.iter().map(|o| o.date.as_str()).collect();
            assert_eq!(dates, ["2026-09-30", "2026-10-30"]);
        }

        #[test]
        fn nao_repete_ocorrencia_ja_materializada() {
            let (id, data) = occurrence(
                SERIES,
                "expense",
                "2026-09",
                "2026-09-30",
                12_000,
                json!({}),
            );
            let ds = dataset_of(vec![
                (RECURRENCES, SERIES, false, series(json!({}))),
                (TRANSACTIONS, &id, false, data),
            ]);
            assert!(upcoming(&ds, "2026-09-24", 30).is_empty());
        }

        #[test]
        fn serie_variavel_projeta_a_media_das_confirmadas_marcada_como_estimada() {
            let (id, data) = occurrence(
                SERIES,
                "expense",
                "2026-08",
                "2026-08-30",
                30_000,
                json!({}),
            );
            let ds = dataset_of(vec![
                (
                    RECURRENCES,
                    SERIES,
                    false,
                    series(json!({ "variable": true, "startOn": "2026-08-30" })),
                ),
                (TRANSACTIONS, &id, false, data),
            ]);
            let found = upcoming(&ds, "2026-09-24", 10);
            assert_eq!(dates_and_amounts(&found), [("2026-09-30", 30_000)]);
            assert!(found[0].estimated);
            let fixed = upcoming(&with(series(json!({}))), "2026-09-24", 10);
            assert!(!fixed[0].estimated);
        }

        #[test]
        fn respeita_pausa_e_data_final() {
            let paused = with(series(json!({ "active": false })));
            assert!(upcoming(&paused, "2026-09-24", 30).is_empty());
            let ended = with(series(json!({ "endOn": "2026-09-29" })));
            assert!(upcoming(&ended, "2026-09-24", 30).is_empty());
        }

        #[test]
        fn ignora_serie_apagada() {
            let ds = dataset_of(vec![(RECURRENCES, SERIES, true, series(json!({})))]);
            assert!(upcoming(&ds, "2026-09-24", 30).is_empty());
        }

        /// `describe("semanal e quinzenal")`, sem os de `nextOccurrences`.
        mod semanal_e_quinzenal {
            use super::*;

            /// A "gravada" do app e so `{...ALIVE, id}`: sem os campos do contrato, no hub ela
            /// e uma linha ignorada, e continua sendo a ocorrencia gravada.
            #[test]
            fn projeta_as_proximas_semanas_sem_as_ja_gravadas() {
                let gravada = stable_entity_id(&occurrence_key(SERIES, "2026-06-10"));
                let ds = dataset_of(vec![
                    (
                        RECURRENCES,
                        SERIES,
                        false,
                        series(json!({ "frequency": "weekly", "startOn": "2026-06-03" })),
                    ),
                    (TRANSACTIONS, &gravada, false, json!({})),
                ]);
                let found = upcoming(&ds, "2026-06-05", 21);
                let dates: Vec<_> = found.iter().map(|o| o.date.as_str()).collect();
                assert_eq!(dates, ["2026-06-17", "2026-06-24"]);
            }

            #[test]
            fn ignora_frequencia_desconhecida() {
                let ds = with(series(json!({ "frequency": "daily" })));
                assert!(upcoming(&ds, "2026-06-05", 60).is_empty());
            }

            /// Decisao da H2a: semanal/quinzenal nao exigem agenda, e a do mes e ignorada
            /// mesmo quando vem.
            #[test]
            fn quinzenal_ignora_a_agenda_do_mes() {
                let ds = with(series(json!({
                    "frequency": "biweekly", "startOn": "2026-06-03",
                    "scheduleType": "nthBusinessDay", "scheduleN": 5
                })));
                let dates: Vec<_> = upcoming(&ds, "2026-06-05", 30)
                    .into_iter()
                    .map(|o| o.date)
                    .collect();
                assert_eq!(dates, ["2026-06-17", "2026-07-01"]);
            }
        }
    }

    /// Casos novos do hub (plano H2b) e o de parcelas de `plan.test.ts` (spec 5, "Parcelas").
    mod projecao_do_hub {
        use super::*;

        fn monthly(over: Value) -> Value {
            series_json(
                json!({
                    "kind": "expense", "description": "Aluguel", "amountMinor": 23_000,
                    "frequency": "monthly", "scheduleType": "dayOfMonth", "scheduleN": 10,
                    "startOn": "2026-06-10", "endOn": null, "active": true
                }),
                over,
            )
        }

        #[test]
        fn dedupe_por_id_estavel_inclui_tombstone_e_linha_ignorada() {
            let (viva, vd) = occurrence("R", "expense", "2026-07", "2026-07-10", 1, json!({}));
            let (apagada, ad) = occurrence("R", "expense", "2026-08", "2026-08-10", 1, json!({}));
            let ignorada = stable_entity_id(&occurrence_key("R", "2026-09"));
            // Data editada pelo usuario: a chave continua a mesma, entao continua deduplicada.
            let (editada, ed) = occurrence("R", "expense", "2026-10", "2026-10-25", 1, json!({}));
            let ds = dataset_of(vec![
                (RECURRENCES, "R", false, monthly(json!({}))),
                (TRANSACTIONS, &viva, false, vd),
                (TRANSACTIONS, &apagada, true, ad),
                (TRANSACTIONS, &ignorada, false, json!({ "kind": "expense" })),
                (TRANSACTIONS, &editada, false, ed),
            ]);
            let periods: Vec<_> = project(&ds, "2026-06-30", "2026-11-30")
                .into_iter()
                .map(|o| o.period)
                .collect();
            assert_eq!(periods, ["2026-11"]);
        }

        #[test]
        fn vencida_sem_linha_entra_na_previsao_e_nao_em_proximas() {
            // Hoje 19/11; a do dia 10 venceu e o celular ainda nao sincronizou a linha.
            let ds = dataset_of(vec![
                (RECURRENCES, "R", false, monthly(json!({}))),
                (
                    RECURRENCES,
                    "S",
                    false,
                    monthly(json!({ "scheduleN": 25, "amountMinor": 5_000 })),
                ),
            ]);
            let mes: Vec<_> = project_month(&ds, "2026-11")
                .into_iter()
                .map(|o| (o.date, o.amount_minor))
                .collect();
            assert_eq!(
                mes,
                [
                    ("2026-11-10".to_string(), 23_000),
                    ("2026-11-25".to_string(), 5_000)
                ]
            );
            let proximas: Vec<_> = upcoming(&ds, "2026-11-19", 30)
                .into_iter()
                .filter(|o| o.date.starts_with("2026-11"))
                .map(|o| o.date)
                .collect();
            assert_eq!(proximas, ["2026-11-25"]);
        }

        #[test]
        fn serie_sem_rule_nao_projeta() {
            let ds = dataset_of(vec![
                (RECURRENCES, "OLD", false, json!({ "frequency": "monthly" })),
                (
                    RECURRENCES,
                    "TORTA",
                    false,
                    monthly(json!({ "amountMinor": "12" })),
                ),
            ]);
            assert!(ds.recurrences.values().all(|r| r.rule.is_none()));
            assert!(project(&ds, "2026-01-01", "2027-12-31").is_empty());
            assert_eq!(
                recurring_month(&ds, "2026-11"),
                RecurringMonth {
                    month: "2026-11".into(),
                    ..RecurringMonth::default()
                }
            );
        }

        /// `plan.test.ts`: "1ª parcela nasce com a sobra de centavos e as seguintes com a
        /// base", reescrito sobre `project` (o hub nao materializa).
        #[test]
        fn primeira_parcela_nasce_com_a_sobra_de_centavos_e_as_seguintes_com_a_base() {
            let ds = dataset_of(vec![(
                RECURRENCES,
                "SERIE-1",
                false,
                json!({
                    "kind": "expense", "description": "Salário", "amountMinor": 3_333,
                    "firstAmountMinor": 3_334, "frequency": "monthly",
                    "scheduleType": "dayOfMonth", "scheduleN": 5, "startOn": "2026-06-05",
                    "endOn": "2026-08-05", "active": true
                }),
            )]);
            let amounts: Vec<_> = project(&ds, "2026-05-31", "2026-12-01")
                .into_iter()
                .map(|o| o.amount_minor)
                .collect();
            assert_eq!(amounts, [3_334, 3_333, 3_333]);
        }

        #[test]
        fn dia_util_projeta_o_n_esimo_dia_util() {
            let ds = dataset_of(vec![(
                RECURRENCES,
                "R",
                false,
                monthly(json!({ "scheduleType": "nthBusinessDay", "scheduleN": 5 })),
            )]);
            let dates: Vec<_> = project(&ds, "2026-07-31", "2026-09-30")
                .into_iter()
                .map(|o| o.date)
                .collect();
            assert_eq!(dates, ["2026-08-07", "2026-09-07"]);
        }

        #[test]
        fn mesma_data_sai_na_ordem_do_id_da_serie() {
            let ds = dataset_of(vec![
                (
                    RECURRENCES,
                    "RPRI",
                    false,
                    monthly(json!({ "scheduleN": 1 })),
                ),
                (
                    RECURRENCES,
                    "RMEL",
                    false,
                    monthly(json!({ "scheduleN": 1 })),
                ),
                (
                    RECURRENCES,
                    "RAAA",
                    false,
                    monthly(json!({ "scheduleN": 1 })),
                ),
            ]);
            for _ in 0..8 {
                let ids: Vec<_> = project(&ds, "2026-11-30", "2026-12-31")
                    .into_iter()
                    .map(|o| o.recurrence_id)
                    .collect();
                assert_eq!(ids, ["RAAA", "RMEL", "RPRI"]);
            }
        }

        #[test]
        fn mes_invalido_nao_projeta() {
            let ds = dataset_of(vec![(RECURRENCES, "R", false, monthly(json!({})))]);
            assert!(project_month(&ds, "2026-13").is_empty());
            assert!(project_month(&ds, "lixo").is_empty());
        }
    }

    /// `free-money.test.ts`.
    mod free_money {
        use super::*;

        const SALARY: &str = "01J9F3K2M7QX8YB4TVWZ0DCEH1";
        const RENT: &str = "01J9F3K2M7QX8YB4TVWZ0DCEH2";

        fn series(over: Value) -> Value {
            series_json(
                json!({
                    "kind": "expense", "description": "Aluguel", "amountMinor": 23_000,
                    "frequency": "monthly", "scheduleType": "dayOfMonth", "scheduleN": 10,
                    "startOn": "2026-06-10", "endOn": null, "active": true
                }),
                over,
            )
        }

        fn salary() -> Value {
            series(json!({
                "kind": "income", "description": "Salário", "amountMinor": 100_000,
                "scheduleN": 5
            }))
        }

        /// Ocorrencia como o `occurrence` do teste do app: valor da serie, data no `scheduleN`.
        fn occ(id: &str, body: &Value, period: &str, over: Value) -> (String, Value) {
            let kind = body["kind"].as_str().unwrap_or("expense");
            let n = body["scheduleN"].as_i64().unwrap_or(1);
            let amount = body["amountMinor"].as_i64().unwrap_or(0);
            occurrence(id, kind, period, &format!("{period}-{n:02}"), amount, over)
        }

        fn month(ds: &Dataset, m: &str) -> RecurringMonth {
            recurring_month(ds, m)
        }

        mod next_month_free {
            use super::*;

            #[test]
            fn entradas_recorrentes_menos_saidas_recorrentes_do_mes_seguinte() {
                let ds = dataset_of(vec![
                    (RECURRENCES, SALARY, false, salary()),
                    (RECURRENCES, RENT, false, series(json!({}))),
                ]);
                assert_eq!(
                    next_month_free(&ds, "2026-11-09"),
                    RecurringMonth {
                        month: "2026-12".into(),
                        income_minor: 100_000,
                        expense_minor: 23_000,
                        free_minor: 77_000,
                        estimated: false,
                    }
                );
            }

            #[test]
            fn vira_o_ano_em_dezembro() {
                let ds = dataset_of(vec![(RECURRENCES, SALARY, false, salary())]);
                assert_eq!(next_month_free(&ds, "2026-12-20").month, "2027-01");
            }

            #[test]
            fn serie_semanal_conta_cada_ocorrencia_do_mes() {
                let weekly = series(json!({
                    "frequency": "weekly", "startOn": "2026-11-02", "amountMinor": 5_000
                }));
                let ds = dataset_of(vec![(RECURRENCES, RENT, false, weekly)]);
                // Segundas de dezembro de 2026: 7, 14, 21 e 28.
                assert_eq!(next_month_free(&ds, "2026-11-09").expense_minor, 20_000);
            }

            #[test]
            fn ignora_serie_pausada_apagada_ou_encerrada_antes_do_mes() {
                let ds = dataset_of(vec![
                    (
                        RECURRENCES,
                        "01J9F3K2M7QX8YB4TVWZ0DCEH3",
                        false,
                        series(json!({ "active": false })),
                    ),
                    (
                        RECURRENCES,
                        "01J9F3K2M7QX8YB4TVWZ0DCEH4",
                        true,
                        series(json!({})),
                    ),
                    (
                        RECURRENCES,
                        "01J9F3K2M7QX8YB4TVWZ0DCEH5",
                        false,
                        series(json!({ "endOn": "2026-11-30" })),
                    ),
                ]);
                assert_eq!(next_month_free(&ds, "2026-11-09").expense_minor, 0);
            }

            fn rent_with_december(over: Value, deleted: bool) -> Dataset {
                let rent = series(json!({}));
                let (id, data) = occ(RENT, &rent, "2026-12", over);
                dataset_of(vec![
                    (RECURRENCES, RENT, false, rent),
                    (TRANSACTIONS, &id, deleted, data),
                ])
            }

            #[test]
            fn conta_uma_vez_a_ocorrencia_que_ja_foi_gravada_com_o_valor_gravado() {
                let ds = rent_with_december(json!({ "amountMinor": 25_000 }), false);
                assert_eq!(next_month_free(&ds, "2026-11-09").expense_minor, 25_000);
            }

            #[test]
            fn ocorrencia_apagada_pelo_usuario_nao_compromete_o_mes() {
                let ds = rent_with_december(json!({}), true);
                assert_eq!(next_month_free(&ds, "2026-11-09").expense_minor, 0);
            }

            #[test]
            fn ocorrencia_gravada_fora_dos_totais_nao_conta() {
                let ds = rent_with_december(json!({ "reportState": "transfer" }), false);
                assert_eq!(next_month_free(&ds, "2026-11-09").expense_minor, 0);
            }

            #[test]
            fn serie_variavel_marca_o_numero_como_estimado() {
                let power = series(json!({ "variable": true, "amountMinor": 18_000 }));
                let ds = dataset_of(vec![(RECURRENCES, RENT, false, power)]);
                let got = next_month_free(&ds, "2026-11-09");
                assert_eq!((got.expense_minor, got.estimated), (18_000, true));
            }

            #[test]
            fn fica_negativo_quando_as_saidas_passam_das_entradas() {
                let ds = dataset_of(vec![(RECURRENCES, RENT, false, series(json!({})))]);
                assert_eq!(next_month_free(&ds, "2026-11-09").free_minor, -23_000);
            }
        }

        mod recurring_month {
            use super::*;

            #[test]
            fn no_mes_corrente_soma_a_ocorrencia_ja_gravada_com_a_que_ainda_vai_nascer_uma_vez_cada()
             {
                // Dia 9 de novembro: o salario do dia 5 ja foi materializado (com o valor que o
                // usuario corrigiu); o aluguel do dia 10 ainda e so regra.
                let s = salary();
                let (id, data) = occ(SALARY, &s, "2026-11", json!({ "amountMinor": 110_000 }));
                let ds = dataset_of(vec![
                    (RECURRENCES, SALARY, false, s),
                    (RECURRENCES, RENT, false, series(json!({}))),
                    (TRANSACTIONS, &id, false, data),
                ]);
                assert_eq!(
                    month(&ds, "2026-11"),
                    RecurringMonth {
                        month: "2026-11".into(),
                        income_minor: 110_000,
                        expense_minor: 23_000,
                        free_minor: 87_000,
                        estimated: false,
                    }
                );
            }

            #[test]
            fn em_mes_passado_conta_so_o_que_esta_gravado_sem_a_regra_somar_de_novo() {
                let s = salary();
                let rent = series(json!({}));
                let (sid, sd) = occ(SALARY, &s, "2026-09", json!({}));
                let (rid, rd) = occ(RENT, &rent, "2026-09", json!({ "amountMinor": 25_000 }));
                let ds = dataset_of(vec![
                    (RECURRENCES, SALARY, false, s),
                    (RECURRENCES, RENT, false, rent),
                    (TRANSACTIONS, &sid, false, sd),
                    (TRANSACTIONS, &rid, false, rd),
                ]);
                let got = month(&ds, "2026-09");
                assert_eq!((got.income_minor, got.expense_minor), (100_000, 25_000));
            }

            #[test]
            fn ocorrencia_apagada_de_um_mes_passado_nao_volta_pela_regra() {
                let s = salary();
                let (id, data) = occ(SALARY, &s, "2026-09", json!({}));
                let ds = dataset_of(vec![
                    (RECURRENCES, SALARY, false, s),
                    (TRANSACTIONS, &id, true, data),
                ]);
                assert_eq!(month(&ds, "2026-09").income_minor, 0);
            }

            #[test]
            fn serie_pausada_ou_apagada_depois_mantem_o_que_ja_tinha_gravado_no_mes() {
                // A renda de setembro existiu; pausar a serie em novembro nao a desfaz.
                let paused = merge(salary(), json!({ "active": false }));
                let gone = series(json!({}));
                let (pid, pd) = occ(SALARY, &paused, "2026-09", json!({}));
                let (gid, gd) = occ(RENT, &gone, "2026-09", json!({}));
                let ds = dataset_of(vec![
                    (RECURRENCES, SALARY, false, paused),
                    (RECURRENCES, RENT, true, gone),
                    (TRANSACTIONS, &pid, false, pd),
                    (TRANSACTIONS, &gid, false, gd),
                ]);
                let got = month(&ds, "2026-09");
                assert_eq!((got.income_minor, got.expense_minor), (100_000, 23_000));
                // Outubro nao foi gravado e a serie nao gera mais: nada.
                assert_eq!(month(&ds, "2026-10").income_minor, 0);
            }

            #[test]
            fn ocorrencia_vencida_ainda_sem_linha_conta_pelo_valor_que_a_materializacao_vai_gravar()
            {
                // Aparelho que ficou fechado: setembro venceu e ninguem materializou.
                let ds = dataset_of(vec![(RECURRENCES, SALARY, false, salary())]);
                assert_eq!(month(&ds, "2026-09").income_minor, 100_000);
            }

            #[test]
            fn mes_anterior_ao_comeco_da_serie_fica_zerado() {
                let ds = dataset_of(vec![
                    (RECURRENCES, SALARY, false, salary()),
                    (RECURRENCES, RENT, false, series(json!({}))),
                ]);
                let got = month(&ds, "2026-05");
                assert_eq!((got.income_minor, got.expense_minor), (0, 0));
            }
        }
    }
}
