// Net Check - a screen test card for the MPC, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Written by card_sources.py from plugin.json. The plugin itself is shared between test cards:
// see shared/net_card_plugin.h.

#include "../../shared/net_card_plugin.h"

class NetCheck : public screen_test::NetCardPlugin
{
public:
    NetCheck() : NetCardPlugin("Net Check", MPC_FOURCC('W', 'N', 'c', '1')) {}   // matches plugin.json "uid"
};

// This macro creates the exported VSTPluginMain entry point. Keep it last.
MPC_PLUGIN_ENTRY(NetCheck)
