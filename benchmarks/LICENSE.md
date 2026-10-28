# Evaluation data licenses

The application's MIT license does not apply to evaluation data or upstream notices.
Evaluation corpora are **not bundled with the app** and are not used to train or
construct its frequency tables.

Both `development.jsonl` and `held-out.jsonl` are collections with these portions:

- Lines 1–4500: derived from **UD English EWT**, CC BY-SA 4.0. Authors/contributors
  include Natalia Silveira, Timothy Dozat, Christopher Manning, Sebastian Schuster,
  Ethan Chi, John Bauer, Miriam Connor, Marie-Catherine de Marneffe, Nathan Schneider,
  Sam Bowman, Hanzhi Zhu and Daniel Galbraith. Full credits/license:
  `licenses/UD_English-EWT-README.md` and `licenses/UD_English-EWT-LICENSE.txt`.
- Lines 4501–9000: derived from **UD Russian SynTagRus**, **CC BY-NC-SA 4.0**.
  Contributors: Kira Droganova, Olga Lyashevskaya and Daniel Zeman. Full source
  description, author credits and license: `licenses/UD_Russian-SynTagRus-README.md`
  and `licenses/UD_Russian-SynTagRus-LICENSE.txt`.

Original text tokens were sampled deterministically; positive examples were
synthetically remapped between keyboard layouts. These changes do not remove
the original data licenses. Source URLs/revisions/checksums and sampling method
are recorded in the adjacent `.meta.json` files. Observe the Russian corpus's
noncommercial restriction when reusing these evaluation records.

`smoke.jsonl` and its handwritten labels are original CodeSwitch test fixtures
under the project's MIT license. Report JSON files contain aggregate measurements
and source/corpus fingerprints, not source text.
