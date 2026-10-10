# Personal highlight rules

Settings → Appearance & memory → Manage highlight rules opens the editor.
This advances AT03 / B4 in the feature coverage plan; regex, badge rules, sounds,
notification policies and ignore/replacement rules remain separate future work.

- Up to 24 rules, shared across workspaces and saved in the existing local profile.
- Match a literal message phrase or an exact Twitch sender login.
- Optional channel restriction; empty means every channel. Invalid restrictions
  are rejected, never silently widened.
- Text rules choose case-sensitive/insensitive and whole-word/substring behavior.
- Enabled/disabled rules and Move up/down priority. The first matching rule chooses
  the accent; five readable pastel accents have matching dark row backgrounds.
- The same colors appear in the transcript and retained Highlights & unread view.
- A local sample-message/sender preview shows matching in the currently selected
  channel without sending anything. Ctrl+S applies the edited rule.
- Switching rules with unsaved edits requires a second explicit selection. Closing
  the dialog keeps the unfinished editor in memory until the app closes. Only saved
  rules persist across app restarts. Delete requires confirmation.

Existing mention/reply highlighting and the eight simple highlight words remain.
Custom rules can choose their color; otherwise these use the original lilac style.
Own messages and deleted messages are excluded. Rule edits reclassify retained
messages within the existing account-session boundary without replaying old-account
attention. The application status bar reports local persistence success or failure.

## Matching and bounds

Sender rules use canonical logins, not editable display names. Channel restrictions
use channel logins, not numeric Twitch IDs. Phrase matching examines ordinary text;
media/emote labels are not searched. Adjacent text fragments join, while emotes
separate phrases. Source identity, text, selection and copy ranges are unchanged.

Case-insensitive matching uses Unicode lowercase, not full case folding or Unicode
normalization. Whole-word matching checks grapheme boundaries and adjacent Unicode
alphanumeric/underscore characters. Combining marks do not create false boundaries;
overlapping candidate phrases are considered. Language-specific word segmentation
and regex matching are not provided by this batch.

Patterns are limited to 80 characters, previews to 500, and channel/sender inputs to
Twitch-login-sized values. Matching happens on arrival or saved configuration change,
not each rendered animation frame. Case-insensitive text segments are normalized
once per message for custom rules. Caches contain retained row IDs and color indices,
not additional copies of chat bodies. Moderation, retention and account changes
reconcile the color cache with the existing highlight lifecycle.

## Verification boundary

The locked Linux app build passes. Focused checks against the actual matching and
attention code covered case, whole words, overlapping matches, combining marks,
exact sender/channel matching, rule priority, invalid channel scopes, own/deleted
messages and color cleanup. This is not Windows UI or live-event acceptance.
The new editor's native Windows workflow and incoming-live-chat colors still need
runtime review. No CI workflow or permanent test UI was added.
