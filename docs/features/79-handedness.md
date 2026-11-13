# Left- and right-handed controls

State: not started

A left- or right-handed choice, kept on each phone or tablet, that moves the on-screen controls
to the side of the hand holding it: the readers' buttons ([38](38-reading-files.md)) and the
floating action buttons on every screen. Page order and tap zones follow the
[reading direction](80-reading-direction.md) instead.

## Build
- [ ] 0.5 h · Texts for the setting — its label, "Left-handed" and "Right-handed" — in a `settings` namespace in `translations/en/frontend/` and `translations/de/frontend/` [BE]
- [ ] 1 h · Keep the choice on the device, right-handed until one is made, outside the session record so it survives logging out and changing servers [FE]
- [ ] 1 h · Provide the choice app-wide: any widget reads the thumb side and rebuilds when it changes [FE]
- [ ] 1 h · Left/right-handed switch on the settings screen ([58](58-account-screens.md)), on phones and tablets only; macOS and Windows keep the right-handed default [FE]
- [ ] 1 h · Floating action buttons sit in the bottom corner on the thumb side, placed by physical side, not by text direction [FE]
- [ ] 1 h · The readers' on-screen buttons sit on the thumb side [FE]
- [ ] 0.5 h · Check by hand on a phone: left-handed puts the reader buttons and the floating buttons on the left, and the choice survives an app restart and a log-out
