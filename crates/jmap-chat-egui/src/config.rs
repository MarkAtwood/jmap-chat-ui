use clap::Parser;
use jmap_chat::{AuthProvider, BasicAuth, BearerAuth, ClientError, CustomCaAuth, NoneAuth};
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
    /// - [`ClientError::InvalidArgument`] if any auth method is combined with `--ca-cert`
    ///   (unsupported combination: `CustomCaAuth` owns the TLS client and cannot inject headers).
    /// - [`ClientError::InvalidArgument`] if `--ca-cert` is provided but the file cannot be read.
    /// - Propagates [`ClientError`] from the underlying auth constructors
    ///   (e.g. empty or invalid token, colon in username).
    pub fn auth_provider(&self) -> Result<Box<dyn AuthProvider>, ClientError> {
        let has_bearer = self.bearer_token.is_some();
        let has_basic_user = self.basic_user.is_some();
        let has_basic_pass = self.basic_pass.is_some();
        let has_ca_cert = self.ca_cert.is_some();

        // Validate all flag combinations before touching the filesystem.
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

        if has_ca_cert && (has_bearer || has_basic_user) {
            return Err(ClientError::InvalidArgument(
                "--ca-cert cannot be combined with --bearer-token or --basic-user/--basic-pass; \
                 CustomCaAuth owns the TLS client and does not support header injection"
                    .into(),
            ));
        }

        // Flags are valid; now read the CA cert file if requested.
        if let Some(token) = &self.bearer_token {
            return Ok(Box::new(BearerAuth::new(token)?));
        }

        if let (Some(user), Some(pass)) = (&self.basic_user, &self.basic_pass) {
            return Ok(Box::new(BasicAuth::new(user, pass)?));
        }

        if let Some(path) = &self.ca_cert {
            let der = std::fs::read(path).map_err(|e| {
                ClientError::InvalidArgument(format!(
                    "cannot read CA certificate '{}': {}",
                    path.display(),
                    e
                ))
            })?;
            return Ok(Box::new(CustomCaAuth::new(der)));
        }

        Ok(Box::new(NoneAuth))
    }
}
