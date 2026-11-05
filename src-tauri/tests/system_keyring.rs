//! Prueba contra el almacén de credenciales REAL del sistema.
//!
//! - Con Secret Service disponible (GNOME Keyring/KWallet): guarda, lee y
//!   borra un valor de prueba en una entrada propia.
//! - Sin él (p. ej., WSL sin llavero): comprueba que el error es descriptivo,
//!   que es lo que la app muestra antes de seguir solo en memoria.
//!
//! ```text
//! cargo test --test system_keyring -- --ignored --nocapture
//! ```

use tiktok_obs_connector_lib::security::{KeyringStore, Secret, TokenStore};

#[test]
#[ignore]
fn keyring_roundtrip_or_descriptive_error() {
    let store = KeyringStore;
    match store.load() {
        Ok(previous) => {
            // Hay almacén: si existe una sesión real guardada, no se toca.
            if previous.is_some() {
                println!("Almacén disponible y con una sesión guardada: no se modifica.");
                return;
            }
            let value = Secret::new("valor-de-prueba-tiktok-obs-connector");
            store.save(&value).expect("guardar");
            assert_eq!(store.load().unwrap().unwrap().expose(), value.expose());
            store.delete().expect("borrar");
            assert!(store.load().unwrap().is_none());
            println!("Almacén del sistema disponible: guardar/leer/borrar correcto.");
        }
        Err(e) => {
            assert!(!e.is_empty());
            println!("Almacén no disponible (la app seguirá solo en memoria): {e}");
        }
    }
}
