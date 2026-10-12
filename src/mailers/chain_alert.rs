use std::time::Duration;

use loco_rs::{app::AppContext, Error, Result};

use crate::{data::explorer::Settings, models::chain_snapshots::Model as Snapshot};

pub struct ChainAlertMailer;
impl ChainAlertMailer {
    /// Send one chain alert through the Mailgun HTTPS API.
    ///
    /// # Errors
    /// When configuration is missing or Mailgun rejects/fails the request.
    pub async fn send_alert(
        ctx: &AppContext,
        to: &str,
        subject: &str,
        detail: &str,
        snapshot: &Snapshot,
    ) -> Result<()> {
        let settings = Settings::from_context(ctx)?;
        let mailgun = &settings.mailgun;
        if mailgun.api_key.trim().is_empty() || mailgun.domain.trim().is_empty() {
            return Err(Error::string(
                "Mailgun requires a non-empty API key and domain",
            ));
        }
        let from = if mailgun.from.trim().is_empty() {
            format!("Catasterism Alerts <alerts@{}>", mailgun.domain)
        } else {
            mailgun.from.clone()
        };

        let body = format!(
            "{detail}\n\npoll ok:      {}\nnetwork:      {}\nchain:        {}\nheight:       {}\nledger check: {}\npeer:         {}\nerror:        {}\n\nexplorer: {}",
            snapshot.ok,
            snapshot.network.as_deref().unwrap_or("unknown"),
            snapshot.chain_id.as_deref().unwrap_or("unknown"),
            snapshot
                .height
                .map_or_else(|| "-".to_string(), |height| height.to_string()),
            snapshot.ledger_check.as_deref().unwrap_or("-"),
            snapshot
                .peer
                .map_or_else(|| "-".to_string(), |peer| peer.to_string()),
            snapshot.error.as_deref().unwrap_or("-"),
            settings.explorer_url,
        );

        let api_url = format!(
            "{}/v3/{}/messages",
            mailgun.api_base_url.trim_end_matches('/'),
            mailgun.domain
        );
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|err| Error::string(&format!("could not build Mailgun client: {err}")))?;
        let response = client
            .post(api_url)
            .basic_auth("api", Some(&mailgun.api_key))
            .form(&[
                ("from", from.as_str()),
                ("to", to),
                ("subject", subject),
                ("text", body.as_str()),
            ])
            .send()
            .await
            .map_err(|err| Error::string(&format!("Mailgun request failed: {err}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::string(&format!(
                "Mailgun returned HTTP {status}: {body}"
            )));
        }

        Ok(())
    }
}
