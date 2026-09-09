//! What deck resolved, on the machine it is run on.
//!
//! Every path here is worked out from environment variables that differ on
//! each platform, and the only honest way to know deck looked in the right
//! place is to ask it where it looked. Windows especially: nobody who can fix
//! this runs it, so `cargo run -p deck-theme --example paths` is what a person
//! there can send back.
//!
//! ```text
//! vscode    here=true  settings=Some("~/Library/Application Support/Code/User/settings.json")
//!           bundled: /Applications/Visual Studio Code.app/Contents/Resources/app/extensions
//! ```

fn main() {
    let home = deck_core::home::home().unwrap();
    for (label, flavour) in [
        ("vscode", deck_theme::vscode::CODE),
        ("cursor", deck_theme::vscode::CURSOR),
        ("windsurf", deck_theme::vscode::WINDSURF),
    ] {
        println!(
            "{label:9} here={:<5} settings={:?}",
            deck_theme::vscode::here(flavour, &home),
            deck_theme::vscode::settings_of(flavour, &home)
        );
        for at in deck_theme::vscode::bundled(flavour)
            .into_iter()
            .filter(|p| p.exists())
        {
            println!("          bundled: {}", at.display());
        }
    }
    println!();
    for editor in deck_theme::Editor::all() {
        let got = deck_theme::read(editor, None, true);
        println!(
            "{:9} installed={:<5} imported={}",
            editor.key(),
            editor.installed(),
            match got {
                Ok(Some(i)) => format!("bg {} accent {}", i.bg.to_hex(), i.accent.to_hex()),
                Ok(None) => "nothing on disk".into(),
                Err(e) => format!("error: {e}"),
            }
        );
    }
}
