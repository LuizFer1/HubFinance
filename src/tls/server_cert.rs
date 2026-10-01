//! Certificado do servidor HTTPS, assinado pela CA local e reemitido quando o IP muda.

use std::net::{IpAddr, Ipv4Addr};

use rcgen::{
    CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, KeyPair,
    KeyUsagePurpose, SanType,
};
use time::{Duration, OffsetDateTime};

use super::TlsError;
use super::ca::{Ca, issuer, whole_seconds};

/// <= 825 dias: exigencia da Apple para certificado de servidor; acima disso o iPhone recusa.
pub const SERVER_DAYS: i64 = 730;
pub const REISSUE_MARGIN_DAYS: i64 = 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerCert {
    pub cert_pem: String,
    pub key_pem: String,
    /// IPs postos no SAN (alem de `127.0.0.1`), guardados em `server.json`.
    pub ips: Vec<Ipv4Addr>,
    pub not_after: OffsetDateTime,
}

pub fn issue_server(
    ca: &Ca,
    ips: &[Ipv4Addr],
    now: OffsetDateTime,
) -> Result<ServerCert, TlsError> {
    let now = whole_seconds(now);
    let issuer = issuer(ca)?;
    let key = KeyPair::generate()?;
    let mut params = CertificateParams::new(vec!["localhost".to_string()])?;
    // 127.0.0.1 sempre: e o que o roteiro de teste e o proprio hub usam na maquina.
    params
        .subject_alt_names
        .push(SanType::IpAddress(IpAddr::V4(Ipv4Addr::LOCALHOST)));
    for ip in ips {
        params
            .subject_alt_names
            .push(SanType::IpAddress(IpAddr::V4(*ip)));
    }
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "HubFinance");
    params.distinguished_name = dn;
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params.not_before = now - Duration::days(1);
    params.not_after = now + Duration::days(SERVER_DAYS);
    let cert = params.signed_by(&key, &issuer)?;
    Ok(ServerCert {
        cert_pem: cert.pem(),
        key_pem: key.serialize_pem(),
        ips: ips.to_vec(),
        not_after: params.not_after,
    })
}

/// IP novo ou validade acabando forcam reemissao. IP que sumiu nao: o celular pode estar do
/// outro lado dessa interface, e reemitir sem ele so quebraria quem ainda o usa.
pub fn needs_reissue(current: &ServerCert, ips: &[Ipv4Addr], now: OffsetDateTime) -> bool {
    let new_ip = ips.iter().any(|ip| !current.ips.contains(ip));
    let expiring = current.not_after - now < Duration::days(REISSUE_MARGIN_DAYS);
    new_ip || expiring
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};

    use super::*;
    use crate::tls::ca::generate_ca;

    fn ips() -> Vec<Ipv4Addr> {
        vec![Ipv4Addr::new(192, 168, 0, 5), Ipv4Addr::new(10, 0, 0, 2)]
    }

    #[test]
    fn emite_cert_que_o_rustls_aceita() {
        let now = OffsetDateTime::now_utc();
        let ca = generate_ca(now).unwrap();
        let server = issue_server(&ca, &ips(), now).unwrap();
        assert_eq!(server.ips, ips());
        assert_eq!(
            server.not_after,
            whole_seconds(now) + Duration::days(SERVER_DAYS)
        );

        let certs: Vec<CertificateDer<'static>> =
            CertificateDer::pem_slice_iter(server.cert_pem.as_bytes())
                .collect::<Result<_, _>>()
                .unwrap();
        let key = PrivateKeyDer::from_pem_slice(server.key_pem.as_bytes()).unwrap();
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(certs, key);
        assert!(config.is_ok());
    }

    #[test]
    fn needs_reissue_por_ip_novo_e_por_validade() {
        let now = OffsetDateTime::now_utc();
        let ca = generate_ca(now).unwrap();
        let server = issue_server(&ca, &ips(), now).unwrap();
        assert!(!needs_reissue(&server, &ips(), now));
        let mut more = ips();
        more.push(Ipv4Addr::new(172, 31, 112, 1));
        assert!(needs_reissue(&server, &more, now));
        // IP a menos nao reemite.
        assert!(!needs_reissue(&server, &ips()[..1], now));
        assert!(needs_reissue(
            &server,
            &ips(),
            server.not_after - Duration::days(29)
        ));
        assert!(!needs_reissue(
            &server,
            &ips(),
            server.not_after - Duration::days(31)
        ));
    }
}
