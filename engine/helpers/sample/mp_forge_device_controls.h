// The MPC Sample's controls, opened so that they hand over the bytes they send and nothing else.
//
// The port of the controls can also hand over every piece wrapped up with the time it came, in blocks padded
// out with zeros, and Akai's own app leaves it doing that. Read as plain bytes, the wrapping looks like
// presses: after any button the zeros read as KNOB FX, pressed about every other time, and the time itself
// reads as whatever it happens to spell. So every program of MP Forge's that opens the controls asks for
// plain bytes first.

#pragma once

#include <cerrno>
#include <cstdio>
#include <cstring>
#include <fcntl.h>

#if defined(__linux__)
#include <sound/asound.h>
#include <sys/ioctl.h>
#endif

namespace device_controls {

#if defined(__linux__)

constexpr size_t kBytesKeptUntilRead = 4096;
constexpr size_t kBytesThatWakeAReader = 1;

// A place that is no port of controls, as when a file stands in for the device, has no wrapping to take off.
inline void askForPlainBytes(int controls) {
    snd_rawmidi_params asked;
    std::memset(&asked, 0, sizeof asked);
    asked.stream = SNDRV_RAWMIDI_STREAM_INPUT;
    asked.buffer_size = kBytesKeptUntilRead;
    asked.avail_min = kBytesThatWakeAReader;
    asked.mode = SNDRV_RAWMIDI_MODE_FRAMING_NONE;
    const bool isRefused = ioctl(controls, SNDRV_RAWMIDI_IOCTL_PARAMS, &asked) < 0 && errno != ENOTTY;
    if (isRefused) std::fprintf(stderr, "The controls could not be asked for plain bytes: %s\n", std::strerror(errno));
}

#else

inline void askForPlainBytes(int /*controls*/) {}

#endif

// The controls at this place, to be read and written. Negative when they could not be opened.
inline int openTheControls(const char* place) {
    const int controls = open(place, O_RDWR | O_NOCTTY | O_CLOEXEC);
    if (controls >= 0) askForPlainBytes(controls);
    return controls;
}

}  // namespace device_controls
