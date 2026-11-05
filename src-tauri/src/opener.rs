//! Apertura de URLs en el navegador predeterminado del sistema.

use crate::app_core::UrlOpener;

pub struct SystemOpener;

impl UrlOpener for SystemOpener {
    fn open(&self, url: &str) -> bool {
        // Solo HTTPS: el frontend nunca decide qué se abre.
        url.starts_with("https://") && open::that_detached(url).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_non_https_urls() {
        assert!(!SystemOpener.open("http://example.com"));
        assert!(!SystemOpener.open("file:///C:/Windows/win.ini"));
        assert!(!SystemOpener.open("javascript:alert(1)"));
    }
}
