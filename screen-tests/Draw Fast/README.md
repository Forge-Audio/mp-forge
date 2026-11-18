# Draw Fast

A screen test card for Akai MPC standalone hardware, by Workbench Tests. It is not a real effect.

- **Sound:** passes the input through. Volume (parameter 0) is the only control that changes the sound.
- **Other parameters:** they give the test screens something to show. See `../shared/` for the plugin's code.
- **Screen:** hand-written files in `skins/`, produced by `../make_test_cards.py`. Do not edit them by hand.
- **CPU:** next to nothing.

What each page tests and what to look for is in `../CHECKLIST.md`.

Needs MPC OS 3.10.1 or later, with External Plugin Support enabled on the MPC
(Preferences, Enhancements (Beta), Enable Enhancements and External Plugin Support).
