//! Descoberta dos IPv4 privados da maquina: o que entra no SAN do certificado e no QR.

use std::net::{IpAddr, Ipv4Addr};

/// `10/8`, `172.16/12`, `192.168/16`. Loopback e link-local ja ficam de fora: nenhum celular
/// alcanca o hub por eles.
pub fn is_private(ip: Ipv4Addr) -> bool {
    ip.is_private()
}

/// Mantem a ordem do SO e descarta repetidos (o mesmo IP pode aparecer em dois adaptadores
/// virtuais). IPv6 fica de fora nesta fatia: o QR e o tutorial falam de um IPv4.
pub fn private_ipv4s(interfaces: &[(String, IpAddr)]) -> Vec<Ipv4Addr> {
    let mut ips = Vec::new();
    for (_, ip) in interfaces {
        if let IpAddr::V4(v4) = ip
            && is_private(*v4)
            && !ips.contains(v4)
        {
            ips.push(*v4);
        }
    }
    ips
}

/// O principal (rota padrao) vai na frente porque e o que entra no QR; se ele nao for um IP
/// privado da lista, a ordem do SO fica como esta.
pub fn order_with_primary(mut ips: Vec<Ipv4Addr>, primary: Option<IpAddr>) -> Vec<Ipv4Addr> {
    if let Some(IpAddr::V4(primary)) = primary
        && let Some(pos) = ips.iter().position(|ip| *ip == primary)
    {
        let ip = ips.remove(pos);
        ips.insert(0, ip);
    }
    ips
}

/// `list_afinet_netifas()` + `local_ip()` (rota padrao) como principal. Erro do SO vira lista
/// vazia: o hub sobe sem rede e o vigia tenta de novo.
pub fn discover() -> Vec<Ipv4Addr> {
    let interfaces = match local_ip_address::list_afinet_netifas() {
        Ok(list) => list,
        Err(err) => {
            tracing::warn!("nao foi possivel listar as interfaces de rede: {err}");
            return Vec::new();
        }
    };
    order_with_primary(
        private_ipv4s(&interfaces),
        local_ip_address::local_ip().ok(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv6Addr;

    fn v4(a: u8, b: u8, c: u8, d: u8) -> Ipv4Addr {
        Ipv4Addr::new(a, b, c, d)
    }

    fn sample() -> Vec<(String, IpAddr)> {
        vec![
            ("lo".into(), IpAddr::V4(v4(127, 0, 0, 1))),
            ("eth".into(), IpAddr::V4(v4(192, 168, 0, 5))),
            ("wan".into(), IpAddr::V4(v4(8, 8, 8, 8))),
            ("ll".into(), IpAddr::V4(v4(169, 254, 1, 1))),
            ("wsl".into(), IpAddr::V4(v4(172, 31, 112, 1))),
            ("v6".into(), IpAddr::V6(Ipv6Addr::LOCALHOST)),
            ("dup".into(), IpAddr::V4(v4(192, 168, 0, 5))),
        ]
    }

    #[test]
    fn filtra_so_privados_na_ordem_sem_repetir() {
        assert_eq!(
            private_ipv4s(&sample()),
            vec![v4(192, 168, 0, 5), v4(172, 31, 112, 1)]
        );
    }

    #[test]
    fn principal_vai_para_a_frente() {
        let ips = private_ipv4s(&sample());
        assert_eq!(
            order_with_primary(ips, Some(IpAddr::V4(v4(172, 31, 112, 1)))),
            vec![v4(172, 31, 112, 1), v4(192, 168, 0, 5)]
        );
    }

    #[test]
    fn principal_fora_da_lista_mantem_a_ordem() {
        let ips = private_ipv4s(&sample());
        assert_eq!(
            order_with_primary(ips.clone(), Some(IpAddr::V4(v4(8, 8, 8, 8)))),
            ips
        );
        assert_eq!(order_with_primary(ips.clone(), None), ips);
    }

    #[test]
    fn faixas_privadas() {
        assert!(is_private(v4(10, 0, 0, 1)));
        assert!(is_private(v4(172, 16, 0, 1)));
        assert!(is_private(v4(172, 31, 255, 255)));
        assert!(is_private(v4(192, 168, 1, 1)));
        assert!(!is_private(v4(172, 32, 0, 1)));
        assert!(!is_private(v4(127, 0, 0, 1)));
        assert!(!is_private(v4(169, 254, 1, 1)));
        assert!(!is_private(v4(8, 8, 8, 8)));
    }
}
