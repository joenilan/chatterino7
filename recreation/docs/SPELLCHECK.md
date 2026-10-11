# Spellcheck (IN11, part)

Settings → Chat → **Spellcheck** (off by default, like Chatterino). English (US).

Misspelled words in the message box appear in a slim "Spelling:" bar above it, with up to 3 suggestions
each (first 3 words). Clicking a suggestion replaces that word. GPUI's text input can't draw
squiggly underlines, so the bar takes their place.

Never flagged: words shorter than 3 letters, words with digits, ALL-CAPS and MixedCase (usually emotes or
names), @mentions, links, `/commands`, emotes known in that channel, names of people who chatted recently,
and common chat words (lol, pog, gg, kekw, ngl, …).

Engine: `spellbook` (pure-Rust Hunspell). Dictionary: SCOWL-derived en_US Hunspell dictionary
(2020.12.07, via wooorm/dictionary-en), bundled in the binary; its license is in
`crates/desktop/assets/dict/LICENSE-en_US.txt` and must ship with releases. It loads on a background thread the first time it's needed.

Evidence: unit test (teh → the; grate, lol, KEKW, @bob and a link not flagged). Live: "teh stream was realy fun lol KEKW"
showed teh and realy with suggestions, and "→ the" fixed the draft.
Not yet: other languages, "add to dictionary".
