//! GPUI Kit's default component icons plus the few extra kit icons Jawjack uses,
//! selected with the kit's `icon_assets!` so only those SVGs are embedded.
use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;

gpui_kit::assets::icon_assets!(ExtraIcons, [Pencil, Shield, RotateCcwClock, Sticker]);

pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut list = gpui_kit::assets::Assets.list(path)?;
        list.extend(ExtraIcons.list(path)?);
        list.sort();
        list.dedup();
        Ok(list)
    }
}
