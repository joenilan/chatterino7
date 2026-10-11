//! GPUI Kit's default icon assets plus the few extra Lucide icons Jawjack uses
//! (copied from gpui-kit-assets, ISC licence in assets/LICENSE-LUCIDE), so every
//! control uses the same icon family without embedding the whole catalog.
use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;

const EXTRA: &[(&str, &[u8])] = &[
    (
        "icons/pencil.svg",
        include_bytes!("../assets/icons/pencil.svg"),
    ),
    (
        "icons/shield.svg",
        include_bytes!("../assets/icons/shield.svg"),
    ),
    (
        "icons/rotate-ccw-clock.svg",
        include_bytes!("../assets/icons/rotate-ccw-clock.svg"),
    ),
    (
        "icons/sticker.svg",
        include_bytes!("../assets/icons/sticker.svg"),
    ),
];

pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = EXTRA.iter().find(|(p, _)| *p == path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        gpui_kit::assets::Assets.load(path)
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut list = gpui_kit::assets::Assets.list(path)?;
        list.extend(
            EXTRA
                .iter()
                .filter(|(p, _)| p.starts_with(path))
                .map(|(p, _)| SharedString::from(*p)),
        );
        Ok(list)
    }
}
