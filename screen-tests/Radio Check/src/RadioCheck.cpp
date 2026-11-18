// Radio Check - a screen test card for the MPC, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Written by card_sources.py from plugin.json. The plugin itself is shared between test cards:
// see shared/radio_card_plugin.h.

#include "../../shared/radio_card_plugin.h"

class RadioCheck : public screen_test::RadioCardPlugin
{
public:
    RadioCheck() : RadioCardPlugin("Radio Check", MPC_FOURCC('W', 'R', 'c', '1')) {}   // matches plugin.json "uid"
};

// This macro creates the exported VSTPluginMain entry point. Keep it last.
MPC_PLUGIN_ENTRY(RadioCheck)
