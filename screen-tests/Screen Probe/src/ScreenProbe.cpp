// Screen Probe - a screen test card for the MPC, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Written by card_sources.py from plugin.json. The plugin itself is shared between test cards:
// see shared/screen_probe_plugin.h.

#include "../../shared/screen_probe_plugin.h"

class ScreenProbe : public screen_test::ScreenProbePlugin
{
public:
    ScreenProbe() : ScreenProbePlugin("Screen Probe", MPC_FOURCC('W', 'S', 'p', '1')) {}   // matches plugin.json "uid"
};

// This macro creates the exported VSTPluginMain entry point. Keep it last.
SCREEN_PROBE_ENTRY(ScreenProbe)
