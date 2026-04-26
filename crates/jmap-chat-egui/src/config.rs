use clap::Parser;
use jmap_chat::auth::{AuthProvider, BasicAuth, BearerAuth, CustomCaAuth, NoneAuth};
use jmap_chat::error::ClientError;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "jmap-chat-egui", about = "JMAP Chat GUI client")]
pub struct Config {
    /// JMAP server base URL (e.g. https://chat.example.com)
    #[arg(long, value_name = "URL")]
    pub server_url: String,

    /// Bearer token for authentication
    #[arg(long, value_name = "TOKEN")]
    pub bearer_token: Option<String>,

    /// Username for Basic authentication
    #[arg(long, value_name = "USER")]
    pub basic_user: Option<String>,

    /// Password for Basic authentication
    #[arg(long, value_name = "PASS")]
    pub basic_pass: Option<String>,

    /// Path to DER-encoded custom CA certificate
    #[arg(long, value_name = "FILE")]
    pub ca_cert: Option<PathBuf>,
}

impl Config {
    /// Build the [`AuthProvider`] described by the parsed CLI flags.
    ///
    /// # Errors
    ///
    /// - [`ClientError::InvalidArgument`] if `--bearer-token` and `--basic-user`/`--basic-pass`
    ///   are both supplied (mutually exclusive).
    /// - [`ClientError::InvalidArgument`] if only one of `--basic-user` / `--basic-pass` is set.
    /// - [`ClientError::InvalidArgument`] if `--ca-cert` is provided but the file cannot be read.
    /// - Propagates [`ClientError`] from the underlying auth constructors
    ///   (e.g. empty or invalid token, colon in username).
    pub fn auth_provider(&self) -> Result<Box<dyn AuthProvider>, ClientError> {
        let has_bearer = self.bearer_token.is_some();
        let has_basic_user = self.basic_user.is_some();
        let has_basic_pass = self.basic_pass.is_some();

        if has_bearer && (has_basic_user || has_basic_pass) {
            return Err(ClientError::InvalidArgument(
                "--bearer-token and --basic-user/--basic-pass are mutually exclusive".into(),
            ));
        }

        if has_basic_user != has_basic_pass {
            return Err(ClientError::InvalidArgument(
                "--basic-user and --basic-pass must both be provided together".into(),
            ));
        }

        let ca_cert_bytes: Option<Vec<u8>> = match &self.ca_cert {
            None => None,
            Some(path) => {
                let bytes = std::fs::read(path).map_err(|e| {
                    ClientError::InvalidArgument(format!(
                        "cannot read CA certificate '{}': {}",
                        path.display(),
                        e
                    ))
                })?;
                Some(bytes)
            }
        };

        if let Some(token) = &self.bearer_token {
            let auth = BearerAuth::new(token)?;
            if let Some(der) = ca_cert_bytes {
                // Bearer + custom CA: not a supported combination in the current auth model.
                // CustomCaAuth owns the client; BearerAuth injects a header.
                // For now, reject this combination with a clear message.
                let _ = der;
                return Err(ClientError::InvalidArgument(
                    "--bearer-token and --ca-cert cannot be used together; \
                     CustomCaAuth does not support injecting an Authorization header"
                        .into(),
                ));
            }
            return Ok(Box::new(auth));
        }

        if let (Some(user), Some(pass)) = (&self.basic_user, &self.basic_pass) {
            let auth = BasicAuth::new(user, pass)?;
            if let Some(der) = ca_cert_bytes {
                let _ = der;
                return Err(ClientError::InvalidArgument(
                    "--basic-user/--basic-pass and --ca-cert cannot be used together; \
                     CustomCaAuth does not support injecting an Authorization header"
                        .into(),
                ));
            }
            return Ok(Box::new(auth));
        }

        if let Some(der) = ca_cert_bytes {
            return Ok(Box::new(CustomCaAuth::new(der)));
        }

        Ok(Box::new(NoneAuth))
    }
}
