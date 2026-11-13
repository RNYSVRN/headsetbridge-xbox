//! The few texts the engine shows to the user itself (the sign-in dialog).
//! Everything else is shown by the desktop app, which reads the English log.
//! The app passes `--lang=de` or `--lang=en`; without it the engine uses English.

use std::sync::atomic::{AtomicBool, Ordering};

static GERMAN: AtomicBool = AtomicBool::new(false);

pub fn set(code: &str) { GERMAN.store(code.to_ascii_lowercase().starts_with("de"), Ordering::Relaxed) }

pub fn is_german() -> bool { GERMAN.load(Ordering::Relaxed) }

/// Title and body of the sign-in dialog.
pub fn sign_in_dialog(url: &str, code: &str) -> (String, String) {
    if is_german() {
        ("HeadsetBridge – Anmeldung".into(),
         format!("Melde dich mit dem Microsoft-Konto deiner Xbox an.\n\n1. Öffne im Browser {url}\n2. Gib diesen Code ein:   {code}\n\nNach der Anmeldung wird die Verbindung automatisch aufgebaut. Die Anmeldung wird auf diesem PC gespeichert."))
    } else {
        ("HeadsetBridge – Sign in".into(),
         format!("Sign in with the Microsoft account of your Xbox.\n\n1. Open {url} in your browser\n2. Enter this code:   {code}\n\nThe connection starts automatically after you sign in. The sign-in is saved on this PC."))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn german_only_for_german_codes() {
        super::set("de-AT");
        assert!(super::is_german());
        assert!(super::sign_in_dialog("u", "C0DE").1.contains("Code ein"));
        super::set("en");
        assert!(!super::is_german());
        assert!(super::sign_in_dialog("u", "C0DE").1.contains("Enter this code"));
    }
}
