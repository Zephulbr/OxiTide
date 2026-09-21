use std::sync::{Mutex, RwLock};

use oxitide_tidal::client::TidalClient;
use oxitide_tidal::pkce::Pkce;

#[derive(Default)]
pub struct AppState {
    pub pending_pkce: Mutex<Option<Pkce>>,
    pub client: RwLock<Option<TidalClient>>,
}

impl AppState {
    pub fn set_pending(&self, pkce: Pkce) {
        *self.pending_pkce.lock().unwrap() = Some(pkce);
    }

    pub fn take_pending(&self) -> Option<Pkce> {
        self.pending_pkce.lock().unwrap().take()
    }

    pub fn is_logged_in(&self) -> bool {
        self.client.read().unwrap().is_some()
    }

    pub fn set_client(&self, client: TidalClient) {
        *self.client.write().unwrap() = Some(client);
    }

    pub fn logout(&self) {
        *self.client.write().unwrap() = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_pkce_is_taken_once() {
        let state = AppState::default();
        state.set_pending(Pkce::from_parts("v".into(), "k".into()));
        assert!(state.take_pending().is_some());
        assert!(state.take_pending().is_none());
    }

    #[test]
    fn logged_in_reflects_client_presence() {
        let state = AppState::default();
        assert!(!state.is_logged_in());
        state.logout();
        assert!(!state.is_logged_in());
    }
}
