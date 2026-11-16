# Nan Effect

An insert effect plugin for Akai MPC standalone hardware, by Workbench Tests.

_Describe what the plugin does here. This file is included in the distributable zip._

## Build

```
python tools/build.py plugins/Nan Effect
python tools/package.py plugins/Nan Effect
```

## Project layout

- `plugin.json` - manifest (name, manufacturer, uid). The `uid` must match the C++ `uniqueId`.
- `src/NanEffect.cpp` - the plugin source (parameters and DSP).
- `ui.json` - which parameters appear on the MPC screen, in which order.
- `build/` - build output (the deployable `Workbench Tests - VST - Nan Effect` folder lives here).
- `dist/` - zips produced by `package.py`.
