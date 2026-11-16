// Draw Live - a screen test card for the MPC, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Written by card_sources.py from plugin.json. The plugin itself is shared between test cards:
// see shared/live_card_plugin.h.

#include "../../shared/live_card_plugin.h"

class DrawLive : public screen_test::LiveCardPlugin
{
public:
    DrawLive() : LiveCardPlugin("Draw Live", MPC_FOURCC('W', 'M', 'w', 'v')) {}   // matches plugin.json "uid"
};

// This macro creates the exported VSTPluginMain entry point. Keep it last.
MPC_PLUGIN_ENTRY(DrawLive)
