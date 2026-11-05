//! Pruebas de contrato contra Streamlabs REAL usando solo datos inválidos.
//!
//! No usan ninguna cuenta ni crean sesiones LIVE: comprueban que los
//! endpoints existen y que la aplicación clasifica bien sus respuestas
//! (redirección del login, 401 del intercambio y del API con token inválido).
//!
//! Marcadas `#[ignore]` porque requieren Internet:
//!
//! ```text
//! cargo test --test streamlabs_contract -- --ignored
//! ```

use virelo_lib::error::AppError;
use virelo_lib::provider::streamlabs::client::ensure_crypto_provider;
use virelo_lib::provider::streamlabs::StreamlabsProvider;
use virelo_lib::provider::{Category, LiveProvider, StartLiveRequest};
use virelo_lib::security::Secret;

fn provider() -> StreamlabsProvider {
    StreamlabsProvider::production().unwrap()
}

#[tokio::test]
#[ignore]
async fn login_url_redirects_to_tiktok_login() {
    ensure_crypto_provider();
    let url = provider().login_url(54321, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let resp = client.get(&url).send().await.unwrap();
    assert!(resp.status().is_redirection(), "estado {}", resp.status());
    let location = resp.headers()["location"].to_str().unwrap().to_string();
    assert!(location.contains("/tiktok/login"), "redirige a {location}");
}

#[tokio::test]
#[ignore]
async fn exchange_with_invalid_code_is_rejected() {
    let err = provider()
        .exchange_code(
            &Secret::new("codigo-invalido"),
            &Secret::new("verifier-invalido"),
        )
        .await
        .unwrap_err();
    assert_eq!(err, AppError::AuthExchangeRejected);
}

#[tokio::test]
#[ignore]
async fn invalid_token_is_detected_on_every_endpoint() {
    let p = provider();
    let t = Secret::new("token-invalido-de-prueba");
    assert_eq!(
        p.account_info(&t).await.unwrap_err(),
        AppError::TokenInvalid
    );
    assert_eq!(
        p.search_categories(&t, "minecraft").await.unwrap_err(),
        AppError::TokenInvalid
    );
    let req = StartLiveRequest {
        title: "prueba de contrato".into(),
        category: Category {
            id: String::new(),
            name: "Otra".into(),
        },
        audience_type: "0".into(),
    };
    assert_eq!(
        p.start_live(&t, &req).await.unwrap_err(),
        AppError::TokenInvalid
    );
    assert_eq!(
        p.end_live(&t, "0").await.unwrap_err(),
        AppError::TokenInvalid
    );
}
