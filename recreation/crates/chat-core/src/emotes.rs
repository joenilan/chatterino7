//! Layout grouping only: no decoding, network fetching, or provider resolution.
use crate::Fragment;

pub const MAX_OVERLAY_LAYERS: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmoteLayer {
    pub provider: String,
    pub id: String,
    pub label: String,
    pub animated: bool,
    pub asset: Option<crate::EmoteAsset>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderRun {
    Text(String),
    /// One inline layout box: first layer reserves width, subsequent layers paint
    /// over it. Copy text retains the original token spelling and separators.
    EmoteStack {
        layers: Vec<EmoteLayer>,
        copy_text: String,
    },
}
impl RenderRun {
    pub fn copy_text(&self) -> &str {
        match self {
            Self::Text(text) => text,
            Self::EmoteStack { copy_text, .. } => copy_text,
        }
    }
}

/// Combine adjacent emotes marked as overlays without altering copy text.
/// Only ASCII spaces/tabs between base and overlay are swallowed into the box;
/// a newline or ordinary text terminates the stack. An orphan overlay is a
/// standalone emote. Overlarge stacks become separate boxes, bounding each draw.
pub fn group_for_layout(fragments: &[Fragment]) -> Vec<RenderRun> {
    let mut runs: Vec<RenderRun> = Vec::new();
    for fragment in fragments {
        match fragment {
            Fragment::Text(text) => runs.push(RenderRun::Text(text.clone())),
            Fragment::Cheer { prefix, label, asset: Some(asset), .. } if label.get(..prefix.len()).is_some_and(|s|s.eq_ignore_ascii_case(prefix)) => {
                runs.push(RenderRun::EmoteStack { layers: vec![EmoteLayer { provider: "twitch-cheer".into(), id: prefix.clone(), label: label[..prefix.len()].into(), animated: true, asset: Some(asset.clone()) }], copy_text: label[..prefix.len()].into() });
                runs.push(RenderRun::Text(label[prefix.len()..].into()));
            }
            Fragment::Gif { label, .. } | Fragment::Cheer { label, .. } | Fragment::Unknown { label, .. } => runs.push(RenderRun::Text(label.clone())),
            Fragment::Emote {
                provider,
                id,
                label,
                overlay,
                animated,
                asset,
            } => {
                let layer = EmoteLayer {
                    provider: provider.clone(),
                    id: id.clone(),
                    label: label.clone(),
                    animated: *animated,
                    asset: asset.clone(),
                };
                let mut stack_index = runs.len();
                if *overlay {
                    while stack_index > 0 {
                        match &runs[stack_index - 1] {
                            RenderRun::Text(text)
                                if text.chars().all(|c| c == ' ' || c == '\t') =>
                            {
                                stack_index -= 1
                            }
                            _ => break,
                        }
                    }
                    if stack_index > 0
                        && matches!(&runs[stack_index - 1], RenderRun::EmoteStack { layers, .. } if layers.len() < MAX_OVERLAY_LAYERS)
                    {
                        let suffix: String = runs
                            .drain(stack_index..)
                            .map(|r| r.copy_text().to_owned())
                            .collect();
                        if let RenderRun::EmoteStack { layers, copy_text } =
                            &mut runs[stack_index - 1]
                        {
                            layers.push(layer);
                            copy_text.push_str(&suffix);
                            copy_text.push_str(label);
                        }
                        continue;
                    }
                }
                runs.push(RenderRun::EmoteStack {
                    layers: vec![layer],
                    copy_text: label.clone(),
                });
            }
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    fn emote(name: &str, overlay: bool) -> Fragment {
        Fragment::Emote {
            provider: "fixture".into(),
            id: name.into(),
            label: name.into(),
            overlay,
            animated: false,
            asset: None,
        }
    }
    fn copy(runs: &[RenderRun]) -> String {
        runs.iter().map(RenderRun::copy_text).collect()
    }
    #[test]
    fn overlays_share_a_box_and_preserve_copy_text() {
        let runs = group_for_layout(&[
            emote("Base", false),
            Fragment::Text(" ".into()),
            emote("Hat", true),
            Fragment::Text("\t".into()),
            emote("Spark", true),
        ]);
        assert_eq!(runs.len(), 1);
        assert_eq!(copy(&runs), "Base Hat\tSpark");
        assert!(matches!(&runs[0], RenderRun::EmoteStack { layers, .. } if layers.len() == 3));
    }
    #[test]
    fn plain_text_breaks_overlay_chain() {
        let runs = group_for_layout(&[
            emote("Base", false),
            Fragment::Text(" hello ".into()),
            emote("Hat", true),
        ]);
        assert_eq!(runs.len(), 3);
        assert_eq!(copy(&runs), "Base hello Hat");
    }
    #[test]
    fn newline_breaks_overlay_chain() {
        let runs = group_for_layout(&[
            emote("Base", false),
            Fragment::Text("\n".into()),
            emote("Hat", true),
        ]);
        assert_eq!(runs.len(), 3);
    }
    #[test]
    fn leading_overlay_and_unicode_are_safe() {
        let runs = group_for_layout(&[Fragment::Text("日本語 👩🏽‍💻 ".into()), emote("孤", true)]);
        assert_eq!(copy(&runs), "日本語 👩🏽‍💻 孤");
        assert_eq!(runs.len(), 2);
    }
    #[test]
    fn layer_count_is_bounded_without_losing_copy() {
        let mut fragments = vec![emote("Base", false)];
        for _ in 0..20 {
            fragments.push(Fragment::Text(" ".into()));
            fragments.push(emote("Hat", true));
        }
        let expected: String = fragments.iter().map(Fragment::copy_text).collect();
        let runs = group_for_layout(&fragments);
        assert_eq!(copy(&runs), expected);
        for run in runs {
            if let RenderRun::EmoteStack { layers, .. } = run {
                assert!(layers.len() <= MAX_OVERLAY_LAYERS);
            }
        }
    }
}
