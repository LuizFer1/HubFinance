//! CA local: o que o celular instala uma vez para confiar em todo certificado do hub.

use rcgen::{
    BasicConstraints, CertificateParams, CidrSubnet, DistinguishedName, DnType, GeneralSubtree,
    IsCa, Issuer, KeyPair, KeyUsagePurpose, NameConstraints,
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
    // A CA fica instalada no celular como raiz de confianca. Sem restricao, quem copiasse
    // `ca.key` deste computador poderia emitir certificado para qualquer site e interceptar o
    // celular fora de casa. Por isso: `pathLen 0` (so assina folhas, nunca outra CA) e nomes
    // restritos ao que o hub usa (`localhost` e IPs privados e de loopback).
    params.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    params.name_constraints = Some(lan_only());
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

/// Faixas privadas (RFC 1918) e loopback, mais `localhost`: as unicas que um certificado
/// emitido por esta CA pode nomear.
fn lan_only() -> NameConstraints {
    let net = |a: u8, b: u8, prefix: u8| {
        GeneralSubtree::IpAddress(CidrSubnet::from_v4_prefix([a, b, 0, 0], prefix))
    };
    NameConstraints {
        permitted_subtrees: vec![
            GeneralSubtree::DnsName("localhost".into()),
            net(10, 0, 8),
            net(172, 16, 12),
            net(192, 168, 16),
            net(127, 0, 8),
        ],
        excluded_subtrees: Vec::new(),
    }
}

/// `Issuer::from_ca_cert_pem` exige a feature `x509-parser` do rcgen (ja no Cargo.toml).
pub fn issuer(ca: &Ca) -> Result<Issuer<'static, KeyPair>, TlsError> {
    let key = KeyPair::from_pem(&ca.key_pem)?;
    Ok(Issuer::from_ca_cert_pem(&ca.cert_pem, key)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::Arc;

    use rcgen::SanType;
    use rustls::client::WebPkiServerVerifier;
    use rustls::client::danger::ServerCertVerifier;
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};

    use crate::tls::server_cert::issue_server;

    fn der(pem: &str) -> CertificateDer<'static> {
        CertificateDer::from_pem_slice(pem.as_bytes()).unwrap()
    }

    /// Verificador de cliente TLS de verdade, com a CA do hub como unica raiz.
    fn verifier(ca: &Ca) -> Arc<WebPkiServerVerifier> {
        let mut roots = rustls::RootCertStore::empty();
        roots.add(der(&ca.cert_pem)).unwrap();
        WebPkiServerVerifier::builder_with_provider(
            Arc::new(roots),
            Arc::new(rustls::crypto::ring::default_provider()),
        )
        .build()
        .unwrap()
    }

    /// Folha assinada pela CA com os SANs pedidos, sem passar por `issue_server` (que sempre
    /// poe `localhost` e `127.0.0.1`).
    fn leaf(ca: &Ca, sans: Vec<SanType>) -> String {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.subject_alt_names = sans;
        params.signed_by(&key, &issuer(ca).unwrap()).unwrap().pem()
    }

    fn verify(ca: &Ca, cert_pem: &str, name: ServerName<'static>) -> Result<(), rustls::Error> {
        verifier(ca)
            .verify_server_cert(&der(cert_pem), &[], &name, &[], UnixTime::now())
            .map(|_| ())
    }

    #[test]
    fn cadeia_real_aceita_o_servidor_e_recusa_nome_fora_da_lan() {
        let now = OffsetDateTime::now_utc();
        let ca = generate_ca(now).unwrap();
        let lan = Ipv4Addr::new(192, 168, 0, 5);
        let server = issue_server(&ca, &[lan], now).unwrap();
        let ip = |ip: Ipv4Addr| ServerName::IpAddress(IpAddr::V4(ip).into());
        assert!(verify(&ca, &server.cert_pem, ip(lan)).is_ok());
        assert!(verify(&ca, &server.cert_pem, ip(Ipv4Addr::LOCALHOST)).is_ok());
        assert!(
            verify(
                &ca,
                &server.cert_pem,
                ServerName::try_from("localhost").unwrap()
            )
            .is_ok()
        );

        // Controle: a mesma folha feita a mao, com IP da LAN, passa. Assim as recusas abaixo
        // vem da restricao de nomes, e nao de algo que falte na folha.
        let other_lan = Ipv4Addr::new(192, 168, 0, 9);
        let ok = leaf(&ca, vec![SanType::IpAddress(IpAddr::V4(other_lan))]);
        assert!(verify(&ca, &ok, ip(other_lan)).is_ok());

        // Mesmo assinada pela CA, folha para IP publico ou dominio de fora e recusada.
        let google = Ipv4Addr::new(8, 8, 8, 8);
        let public_ip = leaf(&ca, vec![SanType::IpAddress(IpAddr::V4(google))]);
        assert!(verify(&ca, &public_ip, ip(google)).is_err());
        let domain = leaf(
            &ca,
            vec![SanType::DnsName("exemplo.com".try_into().unwrap())],
        );
        assert!(verify(&ca, &domain, ServerName::try_from("exemplo.com").unwrap()).is_err());
        // 172.32 fica fora de 172.16/12.
        let edge = Ipv4Addr::new(172, 32, 0, 1);
        let outside = leaf(&ca, vec![SanType::IpAddress(IpAddr::V4(edge))]);
        assert!(verify(&ca, &outside, ip(edge)).is_err());
    }

    #[test]
    fn gera_ca_em_pem_que_carrega_de_volta() {
        let ca = generate_ca(OffsetDateTime::now_utc()).unwrap();
        assert!(ca.cert_pem.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(KeyPair::from_pem(&ca.key_pem).is_ok());
        assert!(issuer(&ca).is_ok());
    }
}
