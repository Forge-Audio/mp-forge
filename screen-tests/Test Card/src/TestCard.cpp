// Test Card - a screen test card for the MPC, by Workbench Tests.
//
// SPDX-License-Identifier: MIT
//
// Written by card_sources.py from plugin.json. The plugin itself is shared between test cards:
// see shared/test_card_plugin.h.

#include "../../shared/test_card_plugin.h"

class TestCard : public screen_test::TestCardPlugin
{
public:
    TestCard() : TestCardPlugin("Test Card", MPC_FOURCC('W', 'a', 'H', 'i')) {}   // matches plugin.json "uid"
};

// This macro creates the exported VSTPluginMain entry point. Keep it last.
MPC_PLUGIN_ENTRY(TestCard)
