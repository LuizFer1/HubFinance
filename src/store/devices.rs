//! Aparelhos pareados. Os timestamps daqui sao relogio de parede do hub, so para exibicao:
//! nunca decidem merge.

use rusqlite::{OptionalExtension, Row};

use super::{Store, StoreError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub device_id: String,
    pub name: String,
    pub key_hash: String,
    /// RFC 3339.
    pub paired_at: String,
    pub last_seen_at: Option<String>,
    pub last_push_at: Option<String>,
    pub last_pull_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub enum Touch {
    Seen,
    Push,
    Pull,
}

impl Store {
    /// Parear de novo o mesmo `device_id` substitui a chave: e o caso "restaurei o backup
    /// no celular novo" — o antigo passa a receber 401.
    ///
    /// A linha inteira e substituida, inclusive `last_*`: a atividade do aparelho antigo nao
    /// descreve o novo.
    pub fn upsert_device(
        &self,
        device_id: &str,
        name: &str,
        key_hash: &str,
        now: &str,
    ) -> Result<Device, StoreError> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO devices (device_id, name, key_hash, paired_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (device_id) DO UPDATE SET
               name = excluded.name, key_hash = excluded.key_hash,
               paired_at = excluded.paired_at, last_seen_at = NULL, last_push_at = NULL,
               last_pull_at = NULL, revoked_at = NULL",
            (device_id, name, key_hash, now),
        )?;
        Ok(conn.query_row(
            &format!("SELECT {COLUMNS} FROM devices WHERE device_id = ?1"),
            [device_id],
            device_from_row,
        )?)
    }

    /// So ativos (`revoked_at IS NULL`). A autenticacao usa `device_by_key_hash_any`, que
    /// tambem ve os revogados; esta fica para os testes conferirem a revogacao.
    #[cfg(test)]
    pub fn device_by_key_hash(&self, key_hash: &str) -> Result<Option<Device>, StoreError> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM devices WHERE key_hash = ?1 AND revoked_at IS NULL"
                ),
                [key_hash],
                device_from_row,
            )
            .optional()?)
    }

    /// Ativo ou revogado. A autenticacao usa esta para distinguir "chave revogada" (vira aviso
    /// na janela) de "chave desconhecida"; o `401` e o mesmo nos dois casos.
    pub fn device_by_key_hash_any(&self, key_hash: &str) -> Result<Option<Device>, StoreError> {
        let conn = self.lock();
        Ok(conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM devices WHERE key_hash = ?1"),
                [key_hash],
                device_from_row,
            )
            .optional()?)
    }

    /// Mais recentes primeiro (`paired_at DESC`).
    pub fn list_devices(&self) -> Result<Vec<Device>, StoreError> {
        let conn = self.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM devices ORDER BY paired_at DESC, device_id"
        ))?;
        let devices = stmt
            .query_map([], device_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(devices)
    }

    /// `true` se o aparelho existe. Revogar de novo mantem a data da primeira revogacao, que
    /// e a que a janela mostra como historico.
    pub fn revoke_device(&self, device_id: &str, now: &str) -> Result<bool, StoreError> {
        let conn = self.lock();
        let changed = conn.execute(
            "UPDATE devices SET revoked_at = COALESCE(revoked_at, ?2) WHERE device_id = ?1",
            (device_id, now),
        )?;
        Ok(changed > 0)
    }

    /// Push e pull tambem contam como "visto": `last_seen_at` e a ultima requisicao
    /// autenticada, qualquer que seja.
    pub fn touch_device(&self, device_id: &str, what: Touch, now: &str) -> Result<(), StoreError> {
        let sql = match what {
            Touch::Seen => "UPDATE devices SET last_seen_at = ?2 WHERE device_id = ?1",
            Touch::Push => {
                "UPDATE devices SET last_seen_at = ?2, last_push_at = ?2 WHERE device_id = ?1"
            }
            Touch::Pull => {
                "UPDATE devices SET last_seen_at = ?2, last_pull_at = ?2 WHERE device_id = ?1"
            }
        };
        self.lock().execute(sql, (device_id, now))?;
        Ok(())
    }
}

const COLUMNS: &str = "device_id, name, key_hash, paired_at, last_seen_at, last_push_at, \
                       last_pull_at, revoked_at";

fn device_from_row(r: &Row<'_>) -> rusqlite::Result<Device> {
    Ok(Device {
        device_id: r.get(0)?,
        name: r.get(1)?,
        key_hash: r.get(2)?,
        paired_at: r.get(3)?,
        last_seen_at: r.get(4)?,
        last_push_at: r.get(5)?,
        last_pull_at: r.get(6)?,
        revoked_at: r.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "01HZZZZZZZZZZZZZZZZZZZZZZA";
    const B: &str = "01HZZZZZZZZZZZZZZZZZZZZZZB";
    const T1: &str = "2026-10-01T18:00:00Z";
    const T2: &str = "2026-10-01T18:05:00Z";
    const T3: &str = "2026-10-01T18:10:00Z";

    fn hash(c: char) -> String {
        c.to_string().repeat(64)
    }

    #[test]
    fn upsert_cria_e_lista() {
        let store = Store::open_in_memory().unwrap();
        let device = store
            .upsert_device(A, "Pixel da Ana", &hash('a'), T1)
            .unwrap();
        assert_eq!(device.device_id, A);
        assert_eq!(device.name, "Pixel da Ana");
        assert_eq!(device.paired_at, T1);
        assert_eq!(device.revoked_at, None);
        assert_eq!(device.last_seen_at, None);
        assert_eq!(store.list_devices().unwrap(), vec![device]);
    }

    #[test]
    fn lista_mais_recentes_primeiro() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_device(A, "A", &hash('a'), T1).unwrap();
        store.upsert_device(B, "B", &hash('b'), T2).unwrap();
        let ids: Vec<String> = store
            .list_devices()
            .unwrap()
            .into_iter()
            .map(|d| d.device_id)
            .collect();
        assert_eq!(ids, vec![B.to_string(), A.to_string()]);
    }

    #[test]
    fn acha_ativo_pelo_hash() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_device(A, "A", &hash('a'), T1).unwrap();
        let found = store.device_by_key_hash(&hash('a')).unwrap();
        assert_eq!(found.map(|d| d.device_id), Some(A.to_string()));
        assert_eq!(store.device_by_key_hash(&hash('f')).unwrap(), None);
    }

    #[test]
    fn revogado_nao_e_achado_mas_continua_na_lista() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_device(A, "A", &hash('a'), T1).unwrap();
        assert!(store.revoke_device(A, T2).unwrap());
        assert_eq!(store.device_by_key_hash(&hash('a')).unwrap(), None);
        let listed = store.list_devices().unwrap();
        assert_eq!(listed[0].revoked_at.as_deref(), Some(T2));
        assert!(!store.revoke_device("inexistente", T2).unwrap());
    }

    #[test]
    fn busca_any_acha_revogado() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_device(A, "A", &hash('a'), T1).unwrap();
        store.revoke_device(A, T2).unwrap();
        let found = store.device_by_key_hash_any(&hash('a')).unwrap().unwrap();
        assert_eq!(found.revoked_at.as_deref(), Some(T2));
        assert_eq!(store.device_by_key_hash_any(&hash('f')).unwrap(), None);
    }

    #[test]
    fn revogar_de_novo_mantem_a_data_original() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_device(A, "A", &hash('a'), T1).unwrap();
        store.revoke_device(A, T2).unwrap();
        assert!(store.revoke_device(A, T3).unwrap());
        assert_eq!(
            store.list_devices().unwrap()[0].revoked_at.as_deref(),
            Some(T2)
        );
    }

    #[test]
    fn reparear_substitui_chave_e_reativa() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_device(A, "Antigo", &hash('a'), T1).unwrap();
        store.touch_device(A, Touch::Push, T1).unwrap();
        store.revoke_device(A, T2).unwrap();
        let device = store.upsert_device(A, "Novo", &hash('b'), T3).unwrap();
        assert_eq!(device.name, "Novo");
        assert_eq!(device.key_hash, hash('b'));
        assert_eq!(device.paired_at, T3);
        assert_eq!(device.revoked_at, None);
        assert_eq!(device.last_push_at, None);
        assert_eq!(store.device_by_key_hash(&hash('a')).unwrap(), None);
        assert_eq!(store.device_by_key_hash(&hash('b')).unwrap(), Some(device));
        assert_eq!(store.list_devices().unwrap().len(), 1);
    }

    #[test]
    fn touch_preenche_os_campos_certos() {
        let store = Store::open_in_memory().unwrap();
        store.upsert_device(A, "A", &hash('a'), T1).unwrap();

        store.touch_device(A, Touch::Seen, T1).unwrap();
        let d = &store.list_devices().unwrap()[0];
        assert_eq!(d.last_seen_at.as_deref(), Some(T1));
        assert_eq!(d.last_push_at, None);
        assert_eq!(d.last_pull_at, None);

        store.touch_device(A, Touch::Push, T2).unwrap();
        let d = &store.list_devices().unwrap()[0];
        assert_eq!(d.last_seen_at.as_deref(), Some(T2));
        assert_eq!(d.last_push_at.as_deref(), Some(T2));
        assert_eq!(d.last_pull_at, None);

        store.touch_device(A, Touch::Pull, T3).unwrap();
        let d = &store.list_devices().unwrap()[0];
        assert_eq!(d.last_seen_at.as_deref(), Some(T3));
        assert_eq!(d.last_push_at.as_deref(), Some(T2));
        assert_eq!(d.last_pull_at.as_deref(), Some(T3));
    }
}
