use super::Platform;
use crate::app::App;

#[derive(Debug)]
pub struct PocketChip;
impl Platform for PocketChip {
    fn apps(&self) -> Result<Vec<App>, String> {
        // Commands/arguments from Marshmallow assets/config.json. Absolute bin
        // locations are conventional Debian paths, pending explicit validation.
        Ok([
            (
                "terminal",
                "Terminal",
                "/usr/bin/vala-terminal",
                vec!["-fs", "8", "-g", "20", "20"],
            ),
            ("pico8", "Play PICO-8", "/usr/bin/pico8", vec![]),
            ("music", "Make Music", "/usr/bin/sunvox", vec![]),
            (
                "help",
                "Get Help",
                "/usr/bin/surf",
                vec!["/usr/share/pocketchip-localdoc/index.html"],
            ),
            ("write", "Write", "/usr/bin/leafpad", vec![]),
            ("files", "Browse Files", "/usr/bin/pcmanfm", vec![]),
        ]
        .into_iter()
        .map(|(id, name, executable, args)| App {
            id: id.into(),
            name: name.into(),
            icon: None,
            executable: executable.into(),
            args: args.into_iter().map(Into::into).collect(),
            cwd: None,
        })
        .collect())
    }
    fn fullscreen(&self) -> bool {
        true
    }
    fn resolution(&self) -> (u16, u16) {
        (480, 272)
    }
}
