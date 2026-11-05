//! Páginas oficiales de TikTok para solicitar y consultar el acceso a LIVE
//! desde PC, por proveedor. Cada proveedor tiene su propio identificador de
//! aplicación en TikTok: la solicitud debe hacerse con el **mismo proveedor**
//! que luego emite las credenciales.
//!
//! Comprobado el 2026-11-05: todas responden HTTP 200 (páginas de TikTok que
//! requieren iniciar sesión; el contenido del formulario no es visible sin ella).

/// Páginas de acceso de un proveedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessPages {
    pub provider: &'static str,
    /// Identificador de la aplicación del proveedor en TikTok.
    pub application_id: &'static str,
    /// Formulario para solicitar el acceso.
    pub apply_url: &'static str,
    /// Página con el estado de la solicitud.
    pub status_url: &'static str,
}

/// Streamlabs (proveedor activo de esta aplicación).
///
/// - `apply_url`: `applicationUrl` de Streamlabs Desktop
///   (`app/services/platforms/tiktok.ts`); con ella se envió con éxito una
///   solicitud real que quedó «En revisión».
/// - `status_url`: página `result` con `apply_mode=8`, la que enlaza el
///   artículo de soporte de Streamlabs «Streaming Access Revoked/Suspended».
pub const STREAMLABS: AccessPages = AccessPages {
    provider: "Streamlabs",
    application_id: "GL6399433079641606942",
    apply_url: "https://www.tiktok.com/falcon/live_g/live_access_pc_apply/intro/index.html?id=GL6399433079641606942&lang=es",
    status_url: "https://www.tiktok.com/falcon/live_g/live_access_pc_apply/result/index.html?id=GL6399433079641606942&lang=es&apply_mode=8",
};

/// StreamElements: referencia para una futura integración. Esta aplicación
/// **no** obtiene credenciales con StreamElements, así que no se usa: solicitar
/// acceso para un proveedor distinto del activo no habilitaría la emisión aquí.
pub const STREAMELEMENTS: AccessPages = AccessPages {
    provider: "StreamElements",
    application_id: "SE7403650104517334022",
    apply_url: "https://www.tiktok.com/falcon/live_g/live_access_pc_apply/intro/index.html?id=SE7403650104517334022&apply_mode=9",
    status_url: "https://www.tiktok.com/falcon/live_g/live_access_pc_apply/result/index.html?id=SE7403650104517334022&apply_mode=9",
};

/// Todas las configuraciones conocidas.
pub const ALL: &[AccessPages] = &[STREAMLABS, STREAMELEMENTS];

/// Páginas del proveedor por nombre (sin distinguir mayúsculas).
pub fn for_provider(name: &str) -> Option<&'static AccessPages> {
    ALL.iter().find(|p| p.provider.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_provider_keeps_its_own_application_id() {
        for p in ALL {
            assert!(p
                .apply_url
                .starts_with("https://www.tiktok.com/falcon/live_g/live_access_pc_apply/"));
            assert!(p
                .status_url
                .starts_with("https://www.tiktok.com/falcon/live_g/live_access_pc_apply/"));
            assert!(
                p.apply_url.contains(&format!("id={}", p.application_id)),
                "{}",
                p.provider
            );
            assert!(
                p.status_url.contains(&format!("id={}", p.application_id)),
                "{}",
                p.provider
            );
        }
        assert_ne!(STREAMLABS.application_id, STREAMELEMENTS.application_id);
    }

    #[test]
    fn documented_parameters_are_preserved() {
        assert!(
            STREAMLABS.status_url.contains("/result/")
                && STREAMLABS.status_url.contains("apply_mode=8")
        );
        assert!(STREAMLABS.apply_url.contains("/intro/"));
        assert!(
            STREAMELEMENTS.apply_url.contains("/intro/")
                && STREAMELEMENTS.apply_url.contains("apply_mode=9")
        );
    }

    #[test]
    fn lookup_by_provider_name() {
        assert_eq!(for_provider("streamlabs"), Some(&STREAMLABS));
        assert_eq!(
            for_provider("StreamElements").map(|p| p.application_id),
            Some("SE7403650104517334022")
        );
        assert_eq!(for_provider("otro"), None);
    }
}
