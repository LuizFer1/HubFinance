//! Textos do modal "Remover": o que cada area apaga, as contagens e o texto do passo de
//! confirmacao. Puros, para as duas interfaces dizerem exatamente a mesma coisa sobre o que
//! vai ser apagado.

// As contagens usam o "4.688" do resto da UI: numero grande sem separador e dificil de ler justo
// no dialogo em que a pessoa decide o que apagar.
use super::shell::{plural, thousands};
use crate::store::erase::{Area, AreaSet, DeviceData};

pub fn area_title(area: Area) -> &'static str {
    match area {
        Area::Budgets => "Metas",
        Area::Transactions => "Transferências",
        Area::Reserves => "Reservas",
        Area::Recurrences => "Recorrências",
    }
}

/// "128 lançamentos", "2 reservas e 14 movimentos": o que a area apaga.
pub fn area_count_text(area: Area, d: &DeviceData) -> String {
    match area {
        Area::Budgets => plural(d.budgets, "meta", "metas"),
        Area::Transactions => plural(d.transactions, "lançamento", "lançamentos"),
        Area::Reserves => format!(
            "{} e {}",
            plural(d.reserves, "reserva", "reservas"),
            plural(d.reserve_movements, "movimento", "movimentos")
        ),
        Area::Recurrences => format!(
            "{} e {}",
            plural(d.recurrences, "série", "séries"),
            plural(d.adjustments, "reajuste", "reajustes")
        ),
    }
}

/// Linhas de apoio de cada area: o que a contagem nao diz sozinha (internas, cascata, o que
/// fica).
pub fn area_notes(area: Area, d: &DeviceData) -> Vec<String> {
    let mut notes = Vec::new();
    match area {
        Area::Budgets => {}
        Area::Transactions => {
            if d.internal_transfers > 0 {
                notes.push(format!(
                    "das quais {} internas",
                    plural(d.internal_transfers, "transferência", "transferências")
                ));
            }
        }
        Area::Reserves => {
            if d.reserves_kept > 0 {
                let kept = if d.reserves_kept == 1 {
                    "1 reserva mantida: tem".to_string()
                } else {
                    format!("{} reservas mantidas: têm", thousands(d.reserves_kept))
                };
                notes.push(format!(
                    "{kept} movimentos de outros aparelhos. Só os movimentos deste aparelho \
                     caem, e isso reduz o saldo da reserva para todos"
                ));
            }
        }
        Area::Recurrences => {
            if d.adjustments_other > 0 {
                notes.push(format!(
                    "+{} de outros aparelhos",
                    plural(d.adjustments_other, "reajuste", "reajustes")
                ));
            }
            if d.generated > 0 {
                notes.push(format!(
                    "{} gerados por estas séries continuam",
                    plural(d.generated, "lançamento", "lançamentos")
                ));
            }
        }
    }
    notes
}

/// "128 lançamentos, 3 metas, 2 reservas com 14 movimentos e 1 recorrência com 2 reajustes".
pub fn selected_list(d: &DeviceData, selected: AreaSet) -> String {
    let mut parts = Vec::new();
    for area in selected.iter() {
        let part = match area {
            Area::Transactions => plural(d.transactions, "lançamento", "lançamentos"),
            Area::Budgets => plural(d.budgets, "meta", "metas"),
            Area::Reserves => format!(
                "{} com {}",
                plural(d.reserves, "reserva", "reservas"),
                plural(d.reserve_movements, "movimento", "movimentos")
            ),
            Area::Recurrences => format!(
                "{} com {}",
                plural(d.recurrences, "recorrência", "recorrências"),
                plural(d.adjustments, "reajuste", "reajustes")
            ),
        };
        parts.push(part);
    }
    match parts.len() {
        0 => String::new(),
        1 => parts.remove(0),
        _ => {
            let last = parts.pop().unwrap_or_default();
            format!("{} e {last}", parts.join(", "))
        }
    }
}

/// Texto em vermelho do passo 2.
pub fn confirm_text(d: &DeviceData, selected: AreaSet) -> String {
    let mut text = format!(
        "Isso apaga {}, também dos outros celulares. Se este aparelho for pareado de novo, ele \
         também perde esses dados. Só se desfaz lançando de novo.",
        selected_list(d, selected)
    );
    // Linha apagada nao e recriada pelo app, mas a serie mantida segue gerando os meses novos.
    if selected.contains(Area::Transactions) && !selected.contains(Area::Recurrences) {
        text.push_str(
            " Séries de recorrência mantidas continuam gerando os próximos meses, mesmo com \
             lançamentos apagados.",
        );
    }
    text
}

/// O botao do passo 2 nomeia a acao: "Remover e apagar 150 itens".
pub fn confirm_label(d: &DeviceData, selected: AreaSet, orphan: bool) -> String {
    let items = plural(d.total_items(selected), "item", "itens");
    if orphan {
        format!("Apagar {items}")
    } else {
        format!("Remover e apagar {items}")
    }
}

/// Texto do toast "Desfazer": cobre a remocao **e** o apagar.
pub fn toast_text(base: &str, d: &DeviceData, selected: AreaSet) -> String {
    if selected.is_empty() {
        return base.to_string();
    }
    let n = d.total_items(selected);
    let verb = if n == 1 {
        "item será apagado"
    } else {
        "itens serão apagados"
    };
    format!("{base} · {} {verb}", thousands(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> DeviceData {
        DeviceData {
            budgets: 3,
            transactions: 128,
            internal_transfers: 4,
            reserves: 2,
            reserve_movements: 14,
            reserves_kept: 1,
            recurrences: 1,
            adjustments: 2,
            adjustments_other: 1,
            generated: 5,
        }
    }

    #[test]
    fn contagem_e_notas_por_area() {
        let d = data();
        assert_eq!(area_count_text(Area::Budgets, &d), "3 metas");
        assert_eq!(area_count_text(Area::Transactions, &d), "128 lançamentos");
        assert_eq!(
            area_count_text(Area::Reserves, &d),
            "2 reservas e 14 movimentos"
        );
        assert_eq!(
            area_count_text(Area::Recurrences, &d),
            "1 série e 2 reajustes"
        );
        assert_eq!(
            area_notes(Area::Transactions, &d),
            vec!["das quais 4 transferências internas"]
        );
        let reserves = area_notes(Area::Reserves, &d);
        assert!(reserves[0].starts_with("1 reserva mantida: tem movimentos de outros aparelhos"));
        assert!(reserves[0].contains("reduz o saldo da reserva para todos"));
        assert_eq!(
            area_notes(Area::Recurrences, &d),
            vec![
                "+1 reajuste de outros aparelhos",
                "5 lançamentos gerados por estas séries continuam"
            ]
        );
        assert!(area_notes(Area::Budgets, &d).is_empty());
    }

    #[test]
    fn contagens_grandes_tem_separador_de_milhar() {
        let d = DeviceData {
            transactions: 4688,
            reserves_kept: 1200,
            ..data()
        };
        assert_eq!(area_count_text(Area::Transactions, &d), "4.688 lançamentos");
        assert!(area_notes(Area::Reserves, &d)[0].starts_with("1.200 reservas mantidas"));
        let mut only = AreaSet::default();
        only.insert(Area::Transactions);
        assert!(confirm_label(&d, only, true).contains("4.688 itens"));
        assert!(toast_text("Dados de Pixel", &d, only).ends_with("4.688 itens serão apagados"));
    }

    #[test]
    fn confirmacao_traz_as_contagens_das_areas_marcadas() {
        let d = data();
        let all = confirm_text(&d, AreaSet::all());
        assert!(
            all.starts_with(
                "Isso apaga 3 metas, 128 lançamentos, 2 reservas com 14 movimentos e 1 \
                 recorrência com 2 reajustes, também dos outros celulares."
            ),
            "{all}"
        );
        assert!(all.contains("pareado de novo, ele também perde esses dados"));
        assert!(!all.contains("continuam gerando"));
        let only_tx: AreaSet = [Area::Transactions].into_iter().collect();
        let text = confirm_text(&d, only_tx);
        assert!(
            text.starts_with("Isso apaga 128 lançamentos, também"),
            "{text}"
        );
        assert!(text.contains("Séries de recorrência mantidas continuam gerando"));
    }

    #[test]
    fn botao_nomeia_a_acao() {
        let d = data();
        assert_eq!(
            confirm_label(&d, AreaSet::all(), false),
            "Remover e apagar 150 itens"
        );
        assert_eq!(confirm_label(&d, AreaSet::all(), true), "Apagar 150 itens");
        let one: AreaSet = [Area::Budgets].into_iter().collect();
        let d1 = DeviceData {
            budgets: 1,
            ..DeviceData::default()
        };
        assert_eq!(confirm_label(&d1, one, false), "Remover e apagar 1 item");
    }

    #[test]
    fn toast_so_fala_de_apagar_se_algo_foi_marcado() {
        let d = data();
        assert_eq!(
            toast_text("Ana foi removida do hub", &d, AreaSet::default()),
            "Ana foi removida do hub"
        );
        assert_eq!(
            toast_text("Ana foi removida do hub", &d, AreaSet::all()),
            "Ana foi removida do hub · 150 itens serão apagados"
        );
    }
}
