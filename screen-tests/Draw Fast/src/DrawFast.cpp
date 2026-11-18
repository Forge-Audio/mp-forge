// Draw Fast - a screen test card for the MPC, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Written by card_sources.py from plugin.json. The plugin itself is shared between test cards:
// see shared/fast_card_plugin.h.

#include "../../shared/fast_card_plugin.h"

class DrawFast : public screen_test::FastCardPlugin
{
public:
    DrawFast() : FastCardPlugin("Draw Fast", MPC_FOURCC('W', 'F', 's', '7')) {}   // matches plugin.json "uid"
};

// This macro creates the exported VSTPluginMain entry point. Keep it last.
MPC_PLUGIN_ENTRY(DrawFast)
