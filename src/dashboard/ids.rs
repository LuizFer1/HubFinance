//! Id deterministico do app (`domain/ids/stable-id.ts::stableEntityId`), portado byte a byte.
//!
//! O hub nunca grava linha, mas precisa *achar* a que o app grava: a ocorrencia de uma serie
//! tem id `stable_entity_id("<serie>:<periodo>")` quando o lugar esta livre. A existencia e
//! decidida pela chave (`OccurrenceIndex::knows_occurrence`); o id so serve para olhar o lugar
//! v1 (linha ignorada ou ambigua). Este id colide (spec 2026-11-19-id-estavel-colisao): nunca
//! use "o id existe" como "o fato existe". Um bit diferente do app e a linha ambigua ou
//! ignorada deixa de ser achada.

/// Mesmo alfabeto Crockford do ULID usado pelo app.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// `stableEntityId(seed)`. O JS itera **unidades UTF-16** (`charCodeAt`), nao bytes nem
/// `char`: por isso `encode_utf16`, e o indice `i` e o da unidade. `Math.imul` e a
/// multiplicacao 32 bits com estouro (`wrapping_mul` em `u32`); `>>> 0` e so a leitura sem sinal,
/// que o `u32` ja e. Os operandos com sinal do JS (`h1 ^ code` etc.) tem os mesmos 32 bits.
pub fn stable_entity_id(seed: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Saidas da funcao TS (M0, `stable-id.vectors.test.ts`, commit `b916129` do app). Nao
    /// mudar para o teste passar: diferenca aqui e bug do port.
    #[test]
    fn stable_entity_id_bate_com_os_vetores_do_app() {
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
            assert_eq!(stable_entity_id(seed), expected, "seed {seed:?}");
        }
    }

    #[test]
    fn ids_da_fixture_grafite() {
        // Spec 3.1.1: as ocorrencias da fixture sao achadas por estes ids.
        assert_eq!(
            stable_entity_id("RSAL:2026-11"),
            "7748FB44FQCR7BWM7Q4RFV4MF7"
        );
        assert_eq!(
            stable_entity_id("RCAD:2026-11"),
            "QZM0FK4WFZC0Q3CWQFMGF34CFF"
        );
    }

    #[test]
    fn sempre_26_caracteres_do_alfabeto() {
        for seed in ["x", "um seed bem mais comprido que os outros", "\u{1F600}"] {
            let id = stable_entity_id(seed);
            assert_eq!(id.len(), 26);
            assert!(id.bytes().all(|c| ALPHABET.contains(&c)), "{id}");
        }
    }
}
