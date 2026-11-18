// The station the Radio Check card plays. No station is kept with the card.
//
// SPDX-License-Identifier: MIT
//
// Copy this file to station_here.h, which git leaves alone, and put a station of your own choosing in it:
// the name of its computer, and the part of its address after that. It has to stream plain MP3 from an
// address that starts with http://, on port 80.

#pragma once

#define RADIO_CHECK_STATION "stream.example.com"
#define RADIO_CHECK_STREAM  "/stream.mp3"
