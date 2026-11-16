// Draw Extras - a screen test card for the MPC, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Written by card_sources.py from plugin.json. The plugin itself is shared between test cards:
// see shared/test_card_plugin.h.

#include "../../shared/test_card_plugin.h"

class DrawExtras : public screen_test::TestCardPlugin
{
public:
    DrawExtras() : TestCardPlugin("Draw Extras", MPC_FOURCC('W', 'q', 'z', 'w')) {}   // matches plugin.json "uid"
};

// This macro creates the exported VSTPluginMain entry point. Keep it last.
MPC_PLUGIN_ENTRY(DrawExtras)
