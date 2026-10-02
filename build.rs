//! Embute o icone e os metadados de versao no executavel do Windows.
//!
//! Sem isso o .exe e os atalhos que o NSIS/MSI criam mostram o icone generico: o
//! cargo-packager poe o icone no instalador, nao no binario.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icon/icon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon/icon.ico")
            .set("ProductName", "HubFinance")
            .set("FileDescription", "HubFinance");
        if let Err(err) = res.compile() {
            // Falhar aqui so tiraria o icone; o build segue com o aviso.
            println!("cargo:warning=icone do executavel nao embutido: {err}");
        }
    }
}
