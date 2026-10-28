# Third-party notices

CodeSwitch's original application code is MIT licensed. Third-party components
retain their own licenses; the MIT license does not relicense bundled data.

## wordfreq

The original English and Russian frequency resources are from **wordfreq**,
Copyright 2022 **Robyn Speer**. The cBpack decoding and single-token frequency
lookup in `crates/engine/src/frequency.rs` are based on the upstream implementation.
That derived code is additionally distributed under Apache-2.0; see
`resources/wordfreq/LICENSE.txt`. Data is distributed under **CC BY-SA 4.0**.
The complete upstream notices, README, implementation reference, immutable source
revision, and resource checksums accompany the resources in `resources/wordfreq/`
and are included in application bundles under `licenses/wordfreq/`.

Source: https://github.com/rspeer/wordfreq

The data includes Google Books Ngrams, Leeds Internet Corpus, Wikipedia,
ParaCrawl, OPUS OpenSubtitles 2018 (credit to OpenSubtitles), and SUBTLEX.
SUBTLEX was created by **Marc Brysbaert et al.** and is **freely available data**:
http://crr.ugent.be/programs-data/subtitle-frequencies.
See upstream `NOTICE.md` and `UPSTREAM-README.md` for complete source attributions.

CC BY-SA 4.0: https://creativecommons.org/licenses/by-sa/4.0/

## Runtime and build dependencies

Tauri and its official plugins are MIT/Apache-2.0; Rust and TypeScript dependencies
retain the licenses specified by their packages. `Cargo.lock` and
`package-lock.json` pin the dependency graph. Release CI exports a dependency
license inventory alongside installers.

## Development-only evaluation data

The evaluation collection under `benchmarks/` retains the separate UD English EWT
(CC BY-SA 4.0) and UD Russian SynTagRus (CC BY-NC-SA 4.0) licenses. It is not
included in app bundles or frequency resources. See `benchmarks/LICENSE.md` for
record ranges, source attributions and modification notes.
