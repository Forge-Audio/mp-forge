// The MPC Sample's one USB socket, turned round for an app that takes a game controller.
//
// The socket usually faces a computer: the device is what is plugged in. A controller is something plugged in
// too, so for one to be read the socket has to face the other way. The device's system has a switch for that,
// and this flips it, on three conditions. The app takes a controller. The switch stands facing a computer. And
// no computer is talking to the device over the socket, so the link to MP Forge is never cut.
//
// The device puts no power on the socket: Akai's published description of its board names nothing that could.
// A controller only speaks once it sees power on the cable, so that has to come from outside, from a hub with
// its own power or an adapter that charges while it connects. A charger on the socket is therefore no reason
// to leave it alone.
//
// Nothing is written to the device's storage: the switch is forgotten when the device restarts. The script
// that starts the app puts it back the way it stood, however the app ends.
//
// This is still being tried. What happened is noted for whoever reads the notes of a run.

#pragma once

#include "mp_forge_host_usb_notes.h"

#include <algorithm>
#include <cerrno>
#include <chrono>
#include <cstdio>
#include <cstring>
#include <fstream>
#include <iterator>
#include <string>
#include <thread>
#include <vector>

namespace mp_forge_host {

class UsbSocket {
public:
    explicit UsbSocket(std::string listing) : listing_(std::move(listing)), switch_(whereTheSwitchIs()) {}

    // True when the socket was turned round.
    bool turnRoundForAController() const {
        if (switch_.empty()) return leftAlone("the device's system has no switch for it");
        if (stands() != kFacingAComputer) return leftAlone("it already stands as " + stands());
        const std::string whyNot = whyItStaysFacingAComputer();
        return whyNot.empty() ? turn() : leftAlone(whyNot);
    }

    std::string stands() const { return system_listing::firstLineOf(switch_); }

private:
    static constexpr char kFacingAComputer[] = "device";
    static constexpr char kTakingWhatIsPluggedIn[] = "host";
    static constexpr const char* kHowALinkInUseStands[] = {"default", "addressed", "configured", "suspended"};

    static bool leftAlone(const std::string& why) {
        std::fprintf(stderr, "The USB socket was not turned round for a game controller: %s.\n", why.c_str());
        return false;
    }

    bool turn() const {
        std::ofstream opened(switch_);
        opened << kTakingWhatIsPluggedIn << std::endl;
        if (!opened.good()) return leftAlone(std::string("the switch could not be flipped (") + std::strerror(errno) + ")");
        std::fprintf(stderr, "The USB socket was turned round to take a game controller.\n");
        return true;
    }

    // The switch of the first socket that has one.
    std::string whereTheSwitchIs() const {
        const std::string sockets = listing_ + "/kernel/debug/usb";
        for (const std::string& socket : system_listing::namesIn(sockets)) {
            const std::string theSwitch = sockets + "/" + socket + "/mode";
            if (!system_listing::firstLineOf(theSwitch).empty()) return theSwitch;
        }
        return {};
    }

    // Nothing, when no computer is talking to the device over the socket.
    std::string whyItStaysFacingAComputer() const {
        const std::string links = listing_ + "/class/udc";
        for (const std::string& link : system_listing::namesIn(links)) {
            const std::string stands = system_listing::firstLineOf(links + "/" + link + "/state");
            if (isHowALinkInUseStands(stands)) return "a computer is talking to the device over it (" + stands + "), and that link is never cut";
        }
        return {};
    }

    // A link stands in one of these ways from the moment a computer has begun to speak to the device, also
    // while that computer lets the link rest.
    static bool isHowALinkInUseStands(const std::string& stands) {
        return std::any_of(std::begin(kHowALinkInUseStands), std::end(kHowALinkInUseStands), [&stands](const char* inUse) { return stands == inUse; });
    }

    std::string listing_;
    std::string switch_;
};

// Only the device has such a socket. Once it is turned round, the system is given a moment to find what is
// plugged in, and what it then lists is noted beside what it listed before.
inline void readyTheUsbSocketForAController() {
#if defined(__linux__)
    const UsbSocket socket("/sys");
    if (!socket.turnRoundForAController()) return;
    std::this_thread::sleep_for(std::chrono::seconds(2));
    std::fprintf(stderr, "Two seconds later the USB socket stands as: %s. What the system lists now:\n", socket.stands().c_str());
    noteUsbAsTheDeviceListsIt();
#endif
}

}  // namespace mp_forge_host
