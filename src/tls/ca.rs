//! CA local: o que o celular instala uma vez para confiar em todo certificado do hub.

use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use time::{Duration, OffsetDateTime};

use super::TlsError;

/// 10 anos: reinstalar a CA no celular e o passo mais chato do pareamento; ela nao deve vencer
/// durante a vida util do hub.
pub const CA_DAYS: i64 = 3650;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ca {
    pub cert_pem: String,
    pub key_pem: String,
}

/// Certificados guardam segundos inteiros; truncar aqui faz o `server.json` dizer exatamente o
/// que esta no certificado.
pub(super) fn whole_seconds(t: OffsetDateTime) -> OffsetDateTime {
    t - Duration::nanoseconds(i64::from(t.nanosecond()))
}

pub fn generate_ca(now: OffsetDateTime) -> Result<Ca, TlsError> {
    let now = whole_seconds(now);
    // ECDSA P-256 (padrao do rcgen): aceito por Android e iOS.
    let key = KeyPair::generate()?;
    let mut params = CertificateParams::new(Vec::<String>::new())?;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "HubFinance CA");
    params.distinguished_name = dn;
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    // -1 dia: relogio do celular atrasado nao pode ver a CA como "ainda nao valida".
    params.not_before = now - Duration::days(1);
    params.not_after = now + Duration::days(CA_DAYS);
    let cert = params.self_signed(&key)?;
    Ok(Ca {
        cert_pem: cert.pem(),
        key_pem: key.serialize_pem(),
    })
}

/// `Issuer::from_ca_cert_pem` exige a feature `x509-parser` do rcgen (ja no Cargo.toml).
pub fn issuer(ca: &Ca) -> Result<Issuer<'static, KeyPair>, TlsError> {
    let key = KeyPair::from_pem(&ca.key_pem)?;
    Ok(Issuer::from_ca_cert_pem(&ca.cert_pem, key)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gera_ca_em_pem_que_carrega_de_volta() {
        let ca = generate_ca(OffsetDateTime::now_utc()).unwrap();
        assert!(ca.cert_pem.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(KeyPair::from_pem(&ca.key_pem).is_ok());
        assert!(issuer(&ca).is_ok());
    }
}
