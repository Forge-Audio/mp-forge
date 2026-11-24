// What the device's own system says of its USB sockets, noted once as an app starts.
//
// A controller that is not found leaves the question of where it got lost: at the socket, at the system's
// drivers, or in this program. These notes answer it for whoever reads the notes of a run. They are read from
// the system's own listing. Nothing is written there, and nothing is switched.

#pragma once

#include <algorithm>
#include <cctype>
#include <cstdio>
#include <dirent.h>
#include <fstream>
#include <iterator>
#include <string>
#include <utility>
#include <vector>

namespace mp_forge_host {

// The device's own system lists what it knows as folders and small files. These read them.
namespace system_listing {

enum class Listed { Everything, LinksOnly };

inline bool isThere(const std::string& folder) {
    DIR* opened = opendir(folder.c_str());
    if (opened != nullptr) closedir(opened);
    return opened != nullptr;
}

inline bool isAFile(const std::string& file) { return std::ifstream(file).good(); }

// In the order of their names, so the notes of two runs can be laid side by side.
inline std::vector<std::string> namesIn(const std::string& folder, Listed listed = Listed::Everything) {
    std::vector<std::string> names;
    DIR* opened = opendir(folder.c_str());
    if (opened == nullptr) return names;
    while (const dirent* entry = readdir(opened))
        if (entry->d_name[0] != '.' && (listed == Listed::Everything || entry->d_type == DT_LNK)) names.emplace_back(entry->d_name);
    closedir(opened);
    std::sort(names.begin(), names.end());
    return names;
}

// What a maker's description holds ends in a byte of nothing, which is left out.
inline std::vector<std::string> linesOf(const std::string& file) {
    std::ifstream opened(file);
    std::vector<std::string> lines;
    for (std::string line; std::getline(opened, line);) {
        line.erase(std::remove(line.begin(), line.end(), '\0'), line.end());
        if (!line.empty()) lines.push_back(line);
    }
    return lines;
}

inline std::string firstLineOf(const std::string& file) {
    const std::vector<std::string> lines = linesOf(file);
    return lines.empty() ? std::string() : lines.front();
}

}  // namespace system_listing

class UsbAsTheSystemListsIt {
public:
    explicit UsbAsTheSystemListsIt(std::string listing) : listing_(std::move(listing)) {}

    void note() const {
        noteWhatIsPluggedIn();
        noteWhichWayTheSocketsFace();
        noteTheLinksToAComputer();
        noteWhatCouldPowerWhatIsPluggedIn();
        noteWhoHoldsThePowerOfTheSocket();
        noteWhatWatchesTheSockets();
        noteWhatFeedsTheDevice();
        for (const char* driver : kDriversOfASocket) noteWhatItHolds(driver);
        noteTheDriversFor("USB", "/bus/usb/drivers");
        noteTheDriversFor("controllers and keyboards", "/bus/hid/drivers");
    }

private:
    using Listed = system_listing::Listed;

    static constexpr const char* kDriversOfASocket[] = {"xhci-hcd", "ehci-platform", "ohci-platform", "dwc3", "dwc2"};
    static constexpr char kLinkToTheDriversOwnCode[] = "module";
    // What the maker's description of a socket holds when the socket can be turned round.
    static constexpr const char* kWhatLetsASocketTurnRound[] = {"usb-role-switch", "extcon", "vbus-supply"};
    // What a supply of power is called when it feeds a USB socket.
    static constexpr const char* kNamesOfPowerForUsb[] = {"usb", "vbus", "otg", "5v"};
    // What the power that goes out of a socket is called, wherever the system lists who may switch it on.
    static constexpr const char* kNamesOfPowerOutOfASocket[] = {"otg", "vbus", "boost"};

    void noteWhatIsPluggedIn() const {
        const std::string devices = listing_ + "/bus/usb/devices";
        int noted = 0;
        for (const std::string& name : system_listing::namesIn(devices, Listed::Everything)) noted += noteIfItIsADevice(devices + "/" + name, name) ? 1 : 0;
        if (noted == 0) std::fprintf(stderr, "The device's system has no USB socket switched on that takes what is plugged into it.\n");
    }

    // The sockets themselves are listed as usb1, usb2 and so on. The parts of a device are listed too, and
    // name no maker.
    static bool noteIfItIsADevice(const std::string& place, const std::string& name) {
        const std::string maker = system_listing::firstLineOf(place + "/idVendor");
        if (maker.empty()) return false;
        const bool isASocket = name.rfind("usb", 0) == 0;
        std::fprintf(stderr, "%s, as the device's system lists it: %s, maker %s, model %s, called \"%s\".\n", isASocket ? "A USB socket" : "Plugged into USB",
                     name.c_str(), maker.c_str(), system_listing::firstLineOf(place + "/idProduct").c_str(), system_listing::firstLineOf(place + "/product").c_str());
        return true;
    }

    void noteWhichWayTheSocketsFace() const {
        const std::string sockets = listing_ + "/class/usb_role";
        const std::vector<std::string> names = system_listing::namesIn(sockets, Listed::Everything);
        if (names.empty()) std::fprintf(stderr, "No USB socket is listed as one that can face either way.\n");
        for (const std::string& name : names)
            std::fprintf(stderr, "The USB socket %s can face either way, and stands as: %s.\n", name.c_str(), system_listing::firstLineOf(sockets + "/" + name + "/role").c_str());
    }

    void noteTheLinksToAComputer() const {
        const std::string links = listing_ + "/class/udc";
        for (const std::string& name : system_listing::namesIn(links, Listed::Everything)) {
            std::fprintf(stderr, "The USB link to a computer (%s) stands as: %s.\n", name.c_str(), system_listing::firstLineOf(links + "/" + name + "/state").c_str());
            noteWhetherItCouldTurnRound(name, links + "/" + name + "/device/of_node");
        }
    }

    // A socket that only ever faces a computer is described so by the device's maker, and the system then has
    // no switch for it. One that can also take what is plugged in has such a switch, and something to give
    // what is plugged in its power.
    void noteWhetherItCouldTurnRound(const std::string& name, const std::string& described) const {
        const std::string faces = system_listing::firstLineOf(described + "/dr_mode");
        std::fprintf(stderr, "Its maker describes it as facing: %s.\n", faces.empty() ? "nothing said" : faces.c_str());
        std::vector<std::string> has;
        for (const char* word : kWhatLetsASocketTurnRound)
            if (system_listing::isAFile(described + "/" + word)) has.emplace_back(word);
        std::fprintf(stderr, "Of what lets a socket turn round, its description has: %s.\n", has.empty() ? "nothing" : inOneLine(has).c_str());
        std::fprintf(stderr, "Everything its description names: %s.\n", inOneLine(system_listing::namesIn(described)).c_str());
        const std::string theSwitch = system_listing::firstLineOf(listing_ + "/kernel/debug/usb/" + name + "/mode");
        std::fprintf(stderr, "The system's own switch for turning it round %s.\n", theSwitch.empty() ? "is not there to be read" : ("stands as: " + theSwitch).c_str());
    }

    void noteWhatCouldPowerWhatIsPluggedIn() const {
        const std::string supplies = listing_ + "/class/regulator";
        for (const std::string& supply : system_listing::namesIn(supplies, Listed::Everything)) {
            const std::string name = system_listing::firstLineOf(supplies + "/" + supply + "/name");
            if (isPowerForUsb(name)) std::fprintf(stderr, "A supply of power called %s stands as: %s.\n", name.c_str(), system_listing::firstLineOf(supplies + "/" + supply + "/state").c_str());
        }
        const std::string cables = listing_ + "/class/extcon";
        for (const std::string& cable : system_listing::namesIn(cables, Listed::Everything))
            std::fprintf(stderr, "What the system sees at a socket (%s): %s.\n", system_listing::firstLineOf(cables + "/" + cable + "/name").c_str(), inOneLine(system_listing::linesOf(cables + "/" + cable + "/state")).c_str());
    }

    static bool isPowerForUsb(const std::string& name) { return namesOneOf(name, std::begin(kNamesOfPowerForUsb), std::end(kNamesOfPowerForUsb)); }

    static bool namesOneOf(std::string name, const char* const* first, const char* const* last) {
        std::transform(name.begin(), name.end(), name.begin(), [](unsigned char letter) { return static_cast<char>(std::tolower(letter)); });
        return std::any_of(first, last, [&name](const char* word) { return name.find(word) != std::string::npos; });
    }

    // The system lists every supply of power with whoever may switch it on under it, further in. The lines of
    // the supplies that go out of a socket are noted as they stand, with all that is under them.
    void noteWhoHoldsThePowerOfTheSocket() const {
        size_t under = std::string::npos;
        for (const std::string& line : system_listing::linesOf(listing_ + "/kernel/debug/regulator/regulator_summary")) {
            const size_t furtherIn = line.find_first_not_of(' ');
            if (under != std::string::npos && furtherIn <= under) under = std::string::npos;
            if (under == std::string::npos && namesOneOf(line, std::begin(kNamesOfPowerOutOfASocket), std::end(kNamesOfPowerOutOfASocket))) under = furtherIn;
            if (under != std::string::npos) std::fprintf(stderr, "Who holds the socket's power, as the system lists it: %s\n", line.c_str());
        }
    }

    // The part of the system that watches a socket may offer a switch of its own, and its maker's description
    // names what powers the socket.
    void noteWhatWatchesTheSockets() const {
        const std::string cables = listing_ + "/class/extcon";
        for (const std::string& cable : system_listing::namesIn(cables)) noteWhatItOffers(system_listing::firstLineOf(cables + "/" + cable + "/name"));
    }

    void noteWhatItOffers(const std::string& watcher) const {
        const std::string place = listing_ + "/bus/platform/devices/" + watcher;
        std::fprintf(stderr, "What watches the socket (%s) offers: %s.\n", watcher.c_str(), inOneLine(system_listing::namesIn(place)).c_str());
        const std::string itsSwitch = system_listing::firstLineOf(place + "/otg_mode");
        if (!itsSwitch.empty()) std::fprintf(stderr, "Its own switch stands as: %s.\n", itsSwitch.c_str());
        for (const std::string& part : system_listing::namesIn(place + "/of_node"))
            if (part.find("port") != std::string::npos)
                std::fprintf(stderr, "Its maker's description of %s names: %s.\n", part.c_str(), inOneLine(system_listing::namesIn(place + "/of_node/" + part)).c_str());
    }

    void noteWhatFeedsTheDevice() const {
        const std::string sources = listing_ + "/class/power_supply";
        for (const std::string& source : system_listing::namesIn(sources))
            std::fprintf(stderr, "A source of power called %s is of the kind %s and stands as: %s.\n", source.c_str(), system_listing::firstLineOf(sources + "/" + source + "/type").c_str(),
                         inOneLine({system_listing::firstLineOf(sources + "/" + source + "/status"), system_listing::firstLineOf(sources + "/" + source + "/online")}).c_str());
    }

    // What a driver holds is listed as links beside it, next to one link to the driver's own code.
    void noteWhatItHolds(const char* driver) const {
        const std::string place = listing_ + "/bus/platform/drivers/" + driver;
        if (!system_listing::isThere(place)) return (void)std::fprintf(stderr, "The system's USB socket driver %s is not there.\n", driver);
        std::vector<std::string> held = system_listing::namesIn(place, Listed::LinksOnly);
        held.erase(std::remove(held.begin(), held.end(), kLinkToTheDriversOwnCode), held.end());
        std::fprintf(stderr, "The system's USB socket driver %s is there and holds %s.\n", driver, held.empty() ? "nothing" : inOneLine(held).c_str());
    }

    void noteTheDriversFor(const char* what, const char* where) const {
        const std::vector<std::string> drivers = system_listing::namesIn(listing_ + where, Listed::Everything);
        std::fprintf(stderr, "The drivers the device's system has for %s: %s.\n", what, drivers.empty() ? "none listed" : inOneLine(drivers).c_str());
    }

    static std::string inOneLine(const std::vector<std::string>& names) {
        std::string line;
        for (const std::string& name : names)
            if (!name.empty()) line += (line.empty() ? "" : ", ") + name;
        return line;
    }

    std::string listing_;
};

// Only the device has such a listing. Anywhere else there is nothing to note.
inline void noteUsbAsTheDeviceListsIt() {
#if defined(__linux__)
    UsbAsTheSystemListsIt("/sys").note();
#endif
}

}  // namespace mp_forge_host
