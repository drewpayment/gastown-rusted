use temporalio_sdk_core::{ClientOptions, RetryClient, Url};

pub async fn connect() -> anyhow::Result<RetryClient<temporalio_sdk_core::Client>> {
    let opts = ClientOptions::builder()
        .target_url(Url::parse("http://localhost:7233")?)
        .client_name("gtr-cli".to_string())
        .client_version(env!("CARGO_PKG_VERSION").to_string())
        .identity("gtr-cli".to_string())
        .build();
    let client = opts.connect("default", None).await?;
    Ok(client)
}
