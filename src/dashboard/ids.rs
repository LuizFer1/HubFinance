//! Ids deterministicos do app (`domain/ids/stable-id.ts`: `stableEntityIdV1` e
//! `stableEntityIdV2`), portados byte a byte.
//!
//! O hub nunca grava linha, mas precisa *achar* a que o app grava: a ocorrencia de uma serie
//! tem id `stable_entity_id_v1("<serie>:<periodo>")` quando o lugar esta livre e
//! `stable_entity_id_v2(...)` quando o v1 ja e de outro fato (spec 2026-11-19-id-estavel-colisao,
//! 4.2). A existencia e decidida pela chave (`OccurrenceIndex::knows_occurrence`); os ids so
//! servem para olhar os lugares (linha ignorada ou ambigua). O v1 colide: nunca use "o id
//! existe" como "o fato existe". Um bit diferente do app e a linha ambigua ou ignorada deixa de
//! ser achada.

use sha2::{Digest, Sha256};

/// Mesmo alfabeto Crockford do ULID usado pelo app.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// `stableEntityIdV1(seed)`, **congelada**: e o id de linhas ja gravadas, dos padroes e da conta
/// comercial; mudar um bit duplicaria tudo isso em todo aparelho. So usa os 5 bits baixos dos
/// hashes, entao tem no maximo 256 valores por comprimento de semente (spec 1.2). O JS itera **unidades UTF-16** (`charCodeAt`), nao bytes nem
/// `char`: por isso `encode_utf16`, e o indice `i` e o da unidade. `Math.imul` e a
/// multiplicacao 32 bits com estouro (`wrapping_mul` em `u32`); `>>> 0` e so a leitura sem sinal,
/// que o `u32` ja e. Os operandos com sinal do JS (`h1 ^ code` etc.) tem os mesmos 32 bits.
pub fn stable_entity_id_v1(seed: &str) -> String {
    let mut h1: u32 = 0x811c_9dc5;
    let mut h2: u32 = 0x811c_9dc5 ^ 0xdead_beef;

    for (i, code) in seed.encode_utf16().enumerate() {
        let code = u32::from(code);
        // `i + 1` cabe em 32 bits para qualquer seed real; o JS tambem o trunca no `imul`.
        let step = (i as u32).wrapping_add(1);
        h1 = (h1 ^ code).wrapping_mul(0x0100_0193);
        h2 = (h2 ^ code).wrapping_mul(0x0100_0193) ^ step.wrapping_mul(0x9e37_79b9);
    }

    let mut a = h1;
    let mut b = h2;
    let mut out = String::with_capacity(26);
    for i in 0..26u32 {
        // No JS `a + b + i * 17` e soma em double (sem estouro), mas `& 31` so olha os 5 bits
        // baixos, que a soma com estouro em 32 bits preserva.
        let index = a.wrapping_add(b).wrapping_add(i * 17) & 31;
        out.push(char::from(ALPHABET[index as usize]));
        a = a.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        // `b` usa o `a` ja atualizado, como no app.
        b = (b ^ a).wrapping_mul(22_695_477).wrapping_add(1);
    }
    out
}

/// `stableEntityIdV2(seed)`: SHA-256 dos bytes **UTF-8** da semente (`TextEncoder` no app;
/// a v1, ao contrario, itera `encode_utf16`), e os 130 primeiros bits do digest lidos do bit
/// mais significativo, 5 a 5, no mesmo alfabeto. O primeiro caractere vai de `0` a `Z`: nao e
/// ULID estrito, e o hub aceita "26 Crockford".
pub fn stable_entity_id_v2(seed: &str) -> String {
    let digest = Sha256::digest(seed.as_bytes());
    let mut out = String::with_capacity(26);
    let mut buffer: u32 = 0;
    let mut bits = 0u32;
    let mut bytes = digest.iter();
    for _ in 0..26 {
        if bits < 5 {
            // 26 * 5 = 130 bits pedem 17 bytes; o digest tem 32.
            let byte = bytes.next().copied().unwrap_or_default();
            // So os `bits` pendentes importam: mascarar evita estourar o `u32` a cada byte.
            buffer = ((buffer & ((1 << bits) - 1)) << 8) | u32::from(byte);
            bits += 8;
        }
        bits -= 5;
        out.push(char::from(ALPHABET[((buffer >> bits) & 31) as usize]));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// Saidas da funcao TS (M0, `stable-id.vectors.test.ts`, commit `b916129` do app). Nao
    /// mudar para o teste passar: diferenca aqui e bug do port.
    #[test]
    fn stable_entity_id_v1_bate_com_os_vetores_do_app() {
        let vectors = [
            ("book:business", "00BQ84BVG8VF8WV30GB78MBBGR"),
            ("R1:2026-11", "CSS401XM49HM8154WS94G1DMM9"),
            ("R1:2026-11-02", "WAV5RJQN4A35G2FNCAB58J7NMA"),
            ("reserve:RE:2026-11", "W75PCK5AM7XPMKDTC7NPWKNA47"),
            ("", "F4Y17GYDQMP1ZG6DZ4E1QGED7M"),
            // `ç` pre-composto (U+00E7, uma unidade UTF-16); `c` + cedilha combinante daria outro.
            ("\u{e7}:2026-11", "AN7MTHFGADQCTSFRA574T1F0AX"),
        ];
        for (seed, expected) in vectors {
            assert_eq!(stable_entity_id_v1(seed), expected, "seed {seed:?}");
        }
    }

    #[test]
    fn ids_da_fixture_grafite() {
        // Spec 3.1.1: as ocorrencias da fixture sao achadas por estes ids.
        assert_eq!(
            stable_entity_id_v1("RSAL:2026-11"),
            "7748FB44FQCR7BWM7Q4RFV4MF7"
        );
        assert_eq!(
            stable_entity_id_v1("RCAD:2026-11"),
            "QZM0FK4WFZC0Q3CWQFMGF34CFF"
        );
    }

    #[test]
    fn sempre_26_caracteres_do_alfabeto() {
        for seed in ["x", "um seed bem mais comprido que os outros", "\u{1F600}"] {
            let id = stable_entity_id_v1(seed);
            assert_eq!(id.len(), 26);
            assert!(id.bytes().all(|c| ALPHABET.contains(&c)), "{id}");
        }
    }

    /// Spec 4.1 / 8, copiados de `V2_VECTORS` (`stable-id.vectors.test.ts`, commit `335a748`
    /// do app em `fix/id-estavel-escritores`), conferidos contra `node:crypto`. Nao mudar para
    /// o teste passar: diferenca aqui e bug do port, e o hub deixaria de achar a linha v2.
    #[test]
    fn stable_entity_id_v2_bate_com_os_vetores_do_app() {
        let vectors = [
            ("", "WERC8GMRZGE196QVYK49JVXS4G"),
            ("book:business", "S184YW4WKYZK3EZ8HGCEN5R9M6"),
            ("R1:2026-11", "9Y2G8QBDMN0DS338QZHY34P0VH"),
            ("R1:2026-11-02", "2Q7RY6VP8Y41BD3G9GAN2AZ61B"),
            ("reserve:RE:2026-11", "N6517GVXA4N8KA1SXK97A125NZ"),
            // `ç` pre-composto: 2 bytes UTF-8 (a v1 le 1 unidade UTF-16).
            ("\u{e7}:2026-11", "TYX61HHBW7FNKTFQ05D8H6JX7H"),
            // 4 bytes UTF-8 contra 2 unidades UTF-16: onde as duas leituras mais divergem.
            ("\u{1F600}", "Y123MD1CBVTMF0X126THQ9BCJE"),
            (
                "budget:01JB000000000000000000000Z",
                "K8GMCRD7YPDMR5T04E3BN375M9",
            ),
        ];
        for (seed, expected) in vectors {
            assert_eq!(stable_entity_id_v2(seed), expected, "seed {seed:?}");
        }
    }

    /// O par verificado da spec (2): mesmo v1, v2 diferentes. E o par que os testes de
    /// `recurrence.rs` usam para a colisao.
    #[test]
    fn par_de_controle_colide_na_v1_e_nao_na_v2() {
        let a = "01JC0000000000000000000001:2027-02";
        let b = "01JC0000000000000000000002:2027-01";
        assert_eq!(stable_entity_id_v1(a), stable_entity_id_v1(b));
        assert_ne!(stable_entity_id_v2(a), stable_entity_id_v2(b));
    }

    #[test]
    fn v2_e_deterministica_e_so_tem_26_crockford() {
        for seed in [
            "x",
            "rec:2026-08",
            "um seed bem mais comprido que os outros",
            "\u{1F600}",
        ] {
            let id = stable_entity_id_v2(seed);
            assert_eq!(id, stable_entity_id_v2(seed));
            assert_eq!(id.len(), 26);
            assert!(id.bytes().all(|c| ALPHABET.contains(&c)), "{id}");
        }
    }

    /// ULIDs realistas e reprodutiveis, o mesmo gerador de `fakeUlids` do app: tempo plausivel
    /// mais 16 caracteres pseudoaleatorios.
    fn fake_ulids(count: usize, salt: u32) -> Vec<String> {
        let mut state: u32 = 0x9e37_79b9 ^ salt;
        let mut next = move || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            usize::try_from(state >> 27).unwrap_or_default()
        };
        (0..count)
            .map(|n| {
                let random: String = (0..16).map(|_| char::from(ALPHABET[next()])).collect();
                format!("01J{n:07}{random}")
            })
            .collect()
    }

    fn months(count: usize) -> Vec<String> {
        (0..count)
            .map(|i| format!("{}-{:02}", 2020 + i / 12, i % 12 + 1))
            .collect()
    }

    /// O defeito que a v2 corrige, fixado para ninguem "consertar" a v1: ela esta congelada.
    /// Os 26 caracteres so dependem dos 5 bits baixos dos dois hashes, e para um mesmo
    /// comprimento de semente sobram 256 ids (spec 1.2).
    #[test]
    fn teto_da_v1_sementes_de_34_caracteres_caem_em_no_maximo_256_ids() {
        let mut ids = BTreeSet::new();
        for series in fake_ulids(400, 1) {
            for month in months(24) {
                let seed = format!("{series}:{month}");
                assert_eq!(seed.len(), 34);
                ids.insert(stable_entity_id_v1(&seed));
            }
        }
        assert!(ids.len() <= 256, "{}", ids.len());
    }

    /// Spec 8: 2 000 series x 120 meses, metas de 1 000 categorias, reajustes e depositos
    /// (comprimentos diferentes) -> nenhum id v2 repetido.
    #[test]
    fn v2_sementes_realistas_distintas_dao_ids_distintos() {
        let series = fake_ulids(2_000, 2);
        let periods = months(120);
        let mut seeds = BTreeSet::new();
        for id in &series {
            for month in &periods {
                seeds.insert(format!("{id}:{month}"));
            }
        }
        for id in fake_ulids(1_000, 3) {
            seeds.insert(format!("budget:{id}"));
        }
        for id in &series[..200] {
            for month in &periods[..24] {
                seeds.insert(format!("{id}:adj:{month}"));
                seeds.insert(format!("reserve:{id}:{month}"));
            }
        }
        assert!(seeds.len() >= 240_000, "{}", seeds.len());
        let ids: BTreeSet<String> = seeds.iter().map(|s| stable_entity_id_v2(s)).collect();
        assert_eq!(ids.len(), seeds.len());
    }
}
