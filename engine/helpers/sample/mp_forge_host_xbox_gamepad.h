// A game controller of the Xbox family, read straight from the MPC Sample's USB socket.
//
// The device's own system has no driver for these controllers, so they never show among what it lists. They
// are spoken to here instead, through what Linux gives every program for reaching a USB device. Two kinds are
// known: the Xbox 360's, and the Xbox One's, which the Series and the Elite controllers speak too.
//
// What a controller says of itself, and what each of its reports means, is read by the plain functions in the
// first half of this file. They need nothing of the device, so they can be tried on any computer. The second
// half finds a controller among what is plugged in, wakes it, and takes its reports as they come.

#pragma once

#include "mp_forge_host_gamepad.h"
#include "mpc_sample_app.h"

#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstring>

#if defined(__linux__)
#include <dirent.h>
#include <linux/usbdevice_fs.h>
#include <string>
#include <sys/ioctl.h>
#endif

namespace mp_forge_host {

// ---------------------------------------------------------------------------------------------------------
// What a controller says of itself, and what its reports mean
// ---------------------------------------------------------------------------------------------------------

enum class XboxKind { None, Xbox360, XboxOne };

// The part of a USB device an Xbox controller is spoken to through: its number, where its reports come from
// and where what it is told goes to.
struct XboxPart {
    XboxKind kind = XboxKind::None;
    unsigned number = 0;
    uint8_t reportsComeFrom = 0;
    uint8_t toldAt = 0;
};

// The maker and the model a USB device says it is.
struct UsbModel {
    unsigned maker = 0;
    unsigned model = 0;
};

namespace usb_described {
constexpr size_t kLengthOfTheDevicesOwn = 18;
constexpr uint8_t kAPart = 4, kAnEndpoint = 5;
constexpr uint8_t kOfItsMakersOwnKind = 0xFF;
constexpr uint8_t kSentInShortBursts = 3, kMaskOfHowItIsSent = 3;
constexpr uint8_t kComesFromTheDevice = 0x80;
}  // namespace usb_described

inline UsbModel usbModelIn(const uint8_t* described, size_t length) {
    if (length < usb_described::kLengthOfTheDevicesOwn) return UsbModel{};
    return UsbModel{static_cast<unsigned>(described[8] | described[9] << 8), static_cast<unsigned>(described[10] | described[11] << 8)};
}

// Each part of a controller says which kind it is by three numbers. These are the two that carry the buttons.
inline XboxKind xboxKindOfAPart(const uint8_t* part) {
    const uint8_t kind = part[5], within = part[6], spoken = part[7];
    if (kind != usb_described::kOfItsMakersOwnKind) return XboxKind::None;
    if (within == 0x5D && spoken == 0x01) return XboxKind::Xbox360;
    if (within == 0x47 && spoken == 0xD0) return XboxKind::XboxOne;
    return XboxKind::None;
}

inline void noteAnEndpointOf(XboxPart& found, const uint8_t* endpoint) {
    const bool isSentInShortBursts = (endpoint[3] & usb_described::kMaskOfHowItIsSent) == usb_described::kSentInShortBursts;
    if (!isSentInShortBursts) return;
    const bool comesFromTheDevice = (endpoint[2] & usb_described::kComesFromTheDevice) != 0;
    if (comesFromTheDevice && found.reportsComeFrom == 0) found.reportsComeFrom = endpoint[2];
    if (!comesFromTheDevice && found.toldAt == 0) found.toldAt = endpoint[2];
}

// Looks through what a USB device says of itself, its own description and then those of its parts, for the
// part an Xbox controller is spoken to through. The first such part is taken, with the endpoints that follow it.
inline XboxPart xboxPartIn(const uint8_t* described, size_t length) {
    XboxPart found;
    for (size_t at = 0; at + 2 <= length && described[at] >= 2 && at + described[at] <= length; at += described[at]) {
        const uint8_t* entry = described + at;
        const bool isAPart = entry[1] == usb_described::kAPart && entry[0] >= 9;
        const bool isAnEndpoint = entry[1] == usb_described::kAnEndpoint && entry[0] >= 7;
        if (isAPart && found.kind != XboxKind::None) break;
        if (isAPart) found = XboxPart{xboxKindOfAPart(entry), entry[2], 0, 0};
        if (isAnEndpoint && found.kind != XboxKind::None) noteAnEndpointOf(found, entry);
    }
    return found.reportsComeFrom != 0 ? found : XboxPart{};
}

namespace xbox_report {

// Where in a report one button is said: which byte, and which bit of it.
struct SaysAButton {
    size_t byte;
    uint8_t bit;
    sample::GamepadButton button;
};

using Button = sample::GamepadButton;

constexpr SaysAButton kOfAnXbox360[] = {
    {2, 0x01, Button::Up}, {2, 0x02, Button::Down}, {2, 0x04, Button::Left}, {2, 0x08, Button::Right},
    {2, 0x10, Button::Start}, {2, 0x20, Button::Select}, {2, 0x40, Button::LeftStickIn}, {2, 0x80, Button::RightStickIn},
    {3, 0x01, Button::LeftShoulder}, {3, 0x02, Button::RightShoulder}, {3, 0x04, Button::Home},
    {3, 0x10, Button::South}, {3, 0x20, Button::East}, {3, 0x40, Button::West}, {3, 0x80, Button::North},
};

constexpr SaysAButton kOfAnXboxOne[] = {
    {4, 0x04, Button::Start}, {4, 0x08, Button::Select},
    {4, 0x10, Button::South}, {4, 0x20, Button::East}, {4, 0x40, Button::West}, {4, 0x80, Button::North},
    {5, 0x01, Button::Up}, {5, 0x02, Button::Down}, {5, 0x04, Button::Left}, {5, 0x08, Button::Right},
    {5, 0x10, Button::LeftShoulder}, {5, 0x20, Button::RightShoulder}, {5, 0x40, Button::LeftStickIn}, {5, 0x80, Button::RightStickIn},
};

constexpr size_t kLengthOfAnXbox360Report = 14, kLengthOfAnXboxOneReport = 18, kLengthOfAHomeReport = 5;
constexpr uint8_t kAnXbox360Report = 0x00, kAnXboxOneReport = 0x20, kAHomeReport = 0x07;
constexpr uint8_t kWantsToBeAnswered = 0x10;
constexpr float kFarthestAStickGoes = 32767.0f, kFarthestAnXbox360TriggerGoes = 255.0f, kFarthestAnXboxOneTriggerGoes = 1023.0f;

// A number sent as two bytes, the low one first.
inline int twoBytesAt(const uint8_t* bytes) { return bytes[0] | bytes[1] << 8; }

// Where a stick stands along one direction, from -1 to 1. Both kinds send up and right as more.
inline float acrossAStickAt(const uint8_t* bytes) {
    const float stands = static_cast<float>(static_cast<int16_t>(twoBytesAt(bytes))) / kFarthestAStickGoes;
    return stands < -1.0f ? -1.0f : stands;
}

template <size_t Count>
void tellOfTheButtons(const SaysAButton (&said)[Count], const uint8_t* report, SaidOfAGamepad& told, HearsTheGamepad& heard) {
    for (const SaysAButton& one : said) told.button(one.button, (report[one.byte] & one.bit) != 0, heard);
}

inline void tellOfTheSticks(const uint8_t* sticks, SaidOfAGamepad& told, HearsTheGamepad& heard) {
    using Axis = sample::GamepadAxis;
    told.stick(Axis::LeftX, acrossAStickAt(sticks), heard);
    told.stick(Axis::LeftY, acrossAStickAt(sticks + 2), heard);
    told.stick(Axis::RightX, acrossAStickAt(sticks + 4), heard);
    told.stick(Axis::RightY, acrossAStickAt(sticks + 6), heard);
}

}  // namespace xbox_report

// One report of an Xbox 360 controller: its buttons, then each trigger in a byte, then the sticks.
inline void tellOfAnXbox360Report(const uint8_t* report, size_t length, SaidOfAGamepad& told, HearsTheGamepad& heard) {
    using namespace xbox_report;
    if (length < kLengthOfAnXbox360Report || report[0] != kAnXbox360Report) return;
    tellOfTheButtons(kOfAnXbox360, report, told, heard);
    told.triggerWithItsButton(sample::GamepadAxis::LeftTrigger, static_cast<float>(report[4]) / kFarthestAnXbox360TriggerGoes, heard);
    told.triggerWithItsButton(sample::GamepadAxis::RightTrigger, static_cast<float>(report[5]) / kFarthestAnXbox360TriggerGoes, heard);
    tellOfTheSticks(report + 6, told, heard);
}

// One report of an Xbox One controller: its buttons, then each trigger in two bytes, then the sticks. The
// button in the middle is said in a report of its own.
inline void tellOfAnXboxOneReport(const uint8_t* report, size_t length, SaidOfAGamepad& told, HearsTheGamepad& heard) {
    using namespace xbox_report;
    if (length >= kLengthOfAHomeReport && report[0] == kAHomeReport) return told.button(Button::Home, (report[4] & 0x01) != 0, heard);
    if (length < kLengthOfAnXboxOneReport || report[0] != kAnXboxOneReport) return;
    tellOfTheButtons(kOfAnXboxOne, report, told, heard);
    told.triggerWithItsButton(sample::GamepadAxis::LeftTrigger, static_cast<float>(twoBytesAt(report + 6)) / kFarthestAnXboxOneTriggerGoes, heard);
    told.triggerWithItsButton(sample::GamepadAxis::RightTrigger, static_cast<float>(twoBytesAt(report + 8)) / kFarthestAnXboxOneTriggerGoes, heard);
    tellOfTheSticks(report + 10, told, heard);
}

// Whether an Xbox One controller waits to be told that its report about the middle button arrived.
inline bool isAHomeReportToAnswer(const uint8_t* report, size_t length) {
    return length >= xbox_report::kLengthOfAHomeReport && report[0] == xbox_report::kAHomeReport && (report[1] & xbox_report::kWantsToBeAnswered) != 0;
}

#if defined(__linux__)

// ---------------------------------------------------------------------------------------------------------
// The controller itself, at the USB socket
// ---------------------------------------------------------------------------------------------------------

class XboxGamepad {
public:
    // What to wait on for news of the controller: it can be written to once a report has come. Nothing, as a
    // wait knows it, while no controller is held.
    int file() const { return file_; }

    // Looks for a controller while none is held, once a second at most.
    void lookForOne() {
        if (file_ >= 0 || Clock::now() < looksAgainAt_) return;
        looksAgainAt_ = Clock::now() + kLooksAgainEvery;
        lookThroughWhatIsPluggedIn();
        isFirstLook_ = false;
    }

    // Takes the report that has come, tells what it says, and asks for the next. A controller that has gone
    // is let go of.
    void hear(HearsTheGamepad& heard) {
        usbdevfs_urb* come = nullptr;
        if (ioctl(file_, USBDEVFS_REAPURBNDELAY, &come) < 0) return errno == EAGAIN ? void() : letGoOfIt(heard);
        if (come->status == 0) tellOf(static_cast<size_t>(come->actual_length), heard);
        if (hasGone(come->status) || !askForAReport()) letGoOfIt(heard);
    }

private:
    using Clock = std::chrono::steady_clock;

    static constexpr auto kLooksAgainEvery = std::chrono::seconds(1);
    static constexpr char kWhereUsbDevicesAre[] = "/dev/bus/usb";
    static constexpr size_t kMostADeviceSaysOfItself = 1024;
    static constexpr unsigned kLongestToTellItSomething = 100;
    static constexpr unsigned kMicrosoft = 0x045E, kXboxOneS = 0x02EA, kEliteSeries2 = 0x0B00;

    void lookThroughWhatIsPluggedIn() {
        DIR* buses = opendir(kWhereUsbDevicesAre);
        if (buses == nullptr) return sayOnceThatUsbCannotBeReached();
        while (const dirent* bus = readdir(buses))
            if (bus->d_name[0] != '.') lookThroughABus(std::string(kWhereUsbDevicesAre) + "/" + bus->d_name);
        closedir(buses);
    }

    void lookThroughABus(const std::string& bus) {
        DIR* devices = opendir(bus.c_str());
        if (devices == nullptr) return;
        while (const dirent* device = readdir(devices))
            if (device->d_name[0] != '.' && file_ < 0) takeIfItIsOne(bus + "/" + device->d_name);
        closedir(devices);
    }

    void sayOnceThatUsbCannotBeReached() const {
        if (isFirstLook_) std::fprintf(stderr, "USB devices cannot be reached at %s (%s), so no controller is read straight from USB.\n", kWhereUsbDevicesAre, std::strerror(errno));
    }

    void takeIfItIsOne(const std::string& place) {
        uint8_t described[kMostADeviceSaysOfItself];
        const int found = open(place.c_str(), O_RDWR | O_CLOEXEC);
        const ssize_t length = found >= 0 ? read(found, described, sizeof described) : -1;
        const XboxPart part = length > 0 ? xboxPartIn(described, static_cast<size_t>(length)) : XboxPart{};
        if (isFirstLook_) sayWhatWasFoundAt(place, found, described, length, part);
        if (part.kind == XboxKind::None || !take(found, part, usbModelIn(described, static_cast<size_t>(length)))) closeIfOpen(found);
    }

    static void closeIfOpen(int found) {
        if (found >= 0) close(found);
    }

    // Every USB device is noted once, so that whoever reads the notes of a run sees what the device saw.
    static void sayWhatWasFoundAt(const std::string& place, int found, const uint8_t* described, ssize_t length, const XboxPart& part) {
        if (found < 0 || length <= 0) return (void)std::fprintf(stderr, "Plugged into USB at %s: it could not be read (%s).\n", place.c_str(), std::strerror(errno));
        const UsbModel model = usbModelIn(described, static_cast<size_t>(length));
        const char* what = part.kind == XboxKind::Xbox360 ? ", an Xbox 360 controller" : part.kind == XboxKind::XboxOne ? ", an Xbox One, Series or Elite controller" : "";
        std::fprintf(stderr, "Plugged into USB: maker %04x, model %04x%s.\n", model.maker, model.model, what);
    }

    bool take(int found, const XboxPart& part, UsbModel model) {
        unsigned number = part.number;
        if (ioctl(found, USBDEVFS_CLAIMINTERFACE, &number) < 0) return sayItCouldNotBeTaken("it is held by something else");
        file_ = found;
        part_ = part;
        if (part.kind == XboxKind::XboxOne) wake(model);
        if (askForAReport()) return sayItWasTaken();
        file_ = -1;
        return sayItCouldNotBeTaken("it gives no reports");
    }

    static bool sayItCouldNotBeTaken(const char* why) {
        std::fprintf(stderr, "The Xbox controller could not be read: %s (%s).\n", why, std::strerror(errno));
        return false;
    }

    static bool sayItWasTaken() {
        std::fprintf(stderr, "The Xbox controller is read straight from USB.\n");
        return true;
    }

    // An Xbox One controller says nothing until it is told to begin. Two later models wait for more.
    void wake(UsbModel model) {
        const uint8_t begin[] = {0x05, 0x20, 0x00, 0x01, 0x00};
        const uint8_t beginForLaterModels[] = {0x05, 0x20, 0x00, 0x0F, 0x06};
        const uint8_t sendEveryButton[] = {0x4D, 0x10, 0x01, 0x02, 0x07, 0x00};
        const bool isALaterModel = model.maker == kMicrosoft && (model.model == kXboxOneS || model.model == kEliteSeries2);
        tell(begin, sizeof begin);
        if (isALaterModel) tell(beginForLaterModels, sizeof beginForLaterModels);
        if (model.maker == kMicrosoft && model.model == kEliteSeries2) tell(sendEveryButton, sizeof sendEveryButton);
    }

    // Each thing the controller is told carries a count in its third byte, one more each time.
    void tell(const uint8_t* bytes, size_t count) {
        if (!send(bytes, count, toldSoFar_++)) std::fprintf(stderr, "The Xbox controller could not be told to begin (%s).\n", std::strerror(errno));
    }

    bool send(const uint8_t* bytes, size_t count, uint8_t counted) {
        uint8_t sentBytes[16];
        if (part_.toldAt == 0 || count > sizeof sentBytes) return false;
        std::memcpy(sentBytes, bytes, count);
        sentBytes[2] = counted;
        usbdevfs_bulktransfer sent{};
        sent.ep = part_.toldAt;
        sent.len = static_cast<unsigned>(count);
        sent.timeout = kLongestToTellItSomething;
        sent.data = sentBytes;
        return ioctl(file_, USBDEVFS_BULK, &sent) >= 0;
    }

    bool askForAReport() {
        std::memset(&asked_, 0, sizeof asked_);
        asked_.type = USBDEVFS_URB_TYPE_INTERRUPT;
        asked_.endpoint = part_.reportsComeFrom;
        asked_.buffer = report_;
        asked_.buffer_length = sizeof report_;
        return ioctl(file_, USBDEVFS_SUBMITURB, &asked_) >= 0;
    }

    static bool hasGone(int status) { return status == -ENODEV || status == -ESHUTDOWN || status == -ENOENT; }

    void tellOf(size_t length, HearsTheGamepad& heard) {
        if (part_.kind == XboxKind::Xbox360) return tellOfAnXbox360Report(report_, length, said_, heard);
        tellOfAnXboxOneReport(report_, length, said_, heard);
        if (isAHomeReportToAnswer(report_, length)) answerTheHomeReport();
    }

    // The answer carries the count of the report it answers.
    void answerTheHomeReport() {
        const uint8_t itArrived[] = {0x01, 0x20, 0x00, 0x09, 0x00, 0x07, 0x20, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00};
        send(itArrived, sizeof itArrived, report_[2]);
    }

    void letGoOfIt(HearsTheGamepad& heard) {
        close(file_);
        file_ = -1;
        said_.everythingLetGo(heard);
        std::fprintf(stderr, "The Xbox controller was unplugged.\n");
    }

    int file_ = -1;
    Clock::time_point looksAgainAt_{};
    bool isFirstLook_ = true;
    XboxPart part_;
    uint8_t toldSoFar_ = 0;
    usbdevfs_urb asked_{};
    uint8_t report_[64] = {};
    SaidOfAGamepad said_;
};

#else

// Only the device has a USB socket to read this way. Anywhere else there is no controller to read here.
class XboxGamepad {
public:
    int file() const { return -1; }
    void lookForOne() {}
    void hear(HearsTheGamepad& /*heard*/) {}
};

#endif

}  // namespace mp_forge_host
