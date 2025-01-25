use std::{io::Cursor, path::PathBuf, sync::Arc};

use tokio::{fs, net::TcpStream};
use tokio_rustls::{
    client::TlsStream,
    rustls::{self, pki_types},
    TlsConnector,
};
use tokio_util::bytes::Bytes;

pub async fn connect<'a>(
    tcp: TcpStream,
    server: &str,
    root_cert_path: Option<&'a PathBuf>,
    client_cert_path: Option<&'a PathBuf>,
    client_key_path: Option<&'a PathBuf>,
) -> Result<TlsStream<TcpStream>, Error> {
    let builder = {
        let mut roots = rustls::RootCertStore::empty();

        let rustls_native_certs::CertificateResult { certs, errors, .. } =
            rustls_native_certs::load_native_certs();
        if !errors.is_empty() {
            errors.iter().for_each(|e| {
                eprintln!("WARNING [rustls-native-certs]: {e}");
            });
        }
        for cert in certs {
            roots.add(cert)?;
        }

        if let Some(cert_path) = root_cert_path {
            let cert_bytes = fs::read(&cert_path).await?;
            let certs = rustls_pemfile::certs(&mut Cursor::new(&cert_bytes))
                .collect::<Result<Vec<_>, _>>()?;
            roots.add_parsable_certificates(certs);
        }

        rustls::ClientConfig::builder().with_root_certificates(roots)
    };

    let client_config = if let Some(cert_path) = client_cert_path {
        let cert_bytes = Bytes::from(fs::read(&cert_path).await?);

        let key_bytes = if let Some(key_path) = client_key_path {
            Bytes::from(fs::read(&key_path).await?)
        } else {
            cert_bytes.clone()
        };

        let certs =
            rustls_pemfile::certs(&mut Cursor::new(&cert_bytes)).collect::<Result<Vec<_>, _>>()?;
        let key = rustls_pemfile::private_key(&mut Cursor::new(&key_bytes))?
            .ok_or(Error::BadPrivateKey)?;

        builder.with_client_auth_cert(certs, key)?
    } else {
        builder.with_no_client_auth()
    };

    let server_name = pki_types::ServerName::try_from(server.to_string())?;

    Ok(TlsConnector::from(Arc::new(client_config))
        .connect(server_name, tcp)
        .await?)
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("rustls error: {0}")]
    Tls(#[from] rustls::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid DNS name: {0}")]
    Dns(#[from] pki_types::InvalidDnsNameError),
    #[error("missing or invalid private key")]
    BadPrivateKey,
    #[error("accept invalid not allowed, please download the server's cert.")]
    NoAcceptInvalid,
}
