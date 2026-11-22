// MP Forge Link: the program MP Forge puts on an MPC Sample, so that the device can be the screen and the
// controls of an app that runs in MP Forge on a computer.
//
// It only carries. Whatever the device's controls send goes to the computer as it is, and the computer sends
// back rows of the picture for the screen. Which control is which is known on the computer alone. While no
// computer is linked the screen says so, and a push of the dial ends the program.
//
//   mp-forge-link <screen> <controls> <port>
//
// The screen is the file a picture is written to, 320 by 240 dots of four bytes each. The controls are the
// port the device's pads, knobs and buttons speak MIDI on. The computer reaches the program at the port.

#include "mpc_sample_app.h"

#include <arpa/inet.h>
#include <cerrno>
#include <csignal>
#include <cstdlib>
#include <fcntl.h>
#include <netinet/in.h>
#include <netinet/tcp.h>
#include <poll.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <unistd.h>
#include <vector>

namespace {

// What the program says first to a computer that reaches it, so that MP Forge knows it has found the program.
const char kGreeting[] = "MP FORGE LINK 1\n";
// The two things a computer sends: rows of the picture for the screen, and bytes to pass on to the controls,
// which is how their lights are set.
constexpr uint8_t kRowsForTheScreen = 'P';
constexpr uint8_t kBytesForTheControls = 'S';

constexpr int kBytesADot = 4;
constexpr size_t kBytesARow = static_cast<size_t>(sample::kScreenWidth) * kBytesADot;
constexpr size_t kMostBytesForTheControls = 1024;
constexpr int kSecondsAComputerMayTakeOverAMessage = 2;
constexpr int kNobody = -1;

// The dial pushed in is a note on the controls' first channel.
constexpr uint8_t kNoteOnTheFirstChannel = 0x90;
constexpr uint8_t kNoteOfTheDial = 111;
constexpr uint8_t kFirstStatusByte = 0x80;
constexpr uint8_t kFirstByteSentInBetween = 0xF8;

const sample::Colour kDark{16, 16, 24};
const sample::Colour kInk{236, 235, 242};
const sample::Colour kSoft{120, 117, 135};
const sample::Colour kAccent{181, 39, 31};

bool readAll(int from, uint8_t* into, size_t count) {
    while (count > 0) {
        const ssize_t arrived = read(from, into, count);
        if (arrived < 0 && errno == EINTR) continue;
        if (arrived <= 0) return false;
        into += arrived;
        count -= static_cast<size_t>(arrived);
    }
    return true;
}

bool writeAll(int to, const uint8_t* bytes, size_t count) {
    while (count > 0) {
        const ssize_t written = write(to, bytes, count);
        if (written < 0 && errno == EINTR) continue;
        if (written <= 0) return false;
        bytes += written;
        count -= static_cast<size_t>(written);
    }
    return true;
}

// A number sent as two bytes, the low one first.
int numberAt(const uint8_t* bytes) { return bytes[0] | bytes[1] << 8; }

// The device's screen. It takes a dot as blue, green, red and a byte it ignores.
class Panel {
public:
    explicit Panel(int file) : file_(file) {}

    void showRows(int firstRow, const uint8_t* rows, size_t byteCount) {
        if (lseek(file_, static_cast<off_t>(firstRow) * static_cast<off_t>(kBytesARow), SEEK_SET) < 0) return;
        writeAll(file_, rows, byteCount);
    }

    void show(const sample::Screen& picture) {
        std::vector<uint8_t> rows(sample::Screen::kByteCount);
        for (size_t dot = 0; dot < rows.size(); dot += kBytesADot) {
            const uint8_t* drawn = picture.bytes() + dot;
            rows[dot] = drawn[2]; rows[dot + 1] = drawn[1]; rows[dot + 2] = drawn[0]; rows[dot + 3] = 0;
        }
        showRows(0, rows.data(), rows.size());
    }

private:
    int file_;
};

// Finds a push of the dial among the bytes the controls send.
class DialPush {
public:
    bool isIn(const uint8_t* bytes, size_t count) {
        bool isFound = false;
        for (size_t at = 0; at < count; ++at) isFound = endsAPush(bytes[at]) || isFound;
        return isFound;
    }

private:
    bool endsAPush(uint8_t byte) {
        if (byte >= kFirstByteSentInBetween) return false;
        if (byte >= kFirstStatusByte) return startAMessage(byte);
        if (status_ != kNoteOnTheFirstChannel) return false;
        if (note_ == kNoNoteYet) return takeTheNote(byte);
        const bool isAPush = note_ == kNoteOfTheDial && byte > 0;
        note_ = kNoNoteYet;
        return isAPush;
    }

    bool startAMessage(uint8_t status) {
        status_ = status;
        note_ = kNoNoteYet;
        return false;
    }

    bool takeTheNote(uint8_t note) {
        note_ = note;
        return false;
    }

    static constexpr int kNoNoteYet = -1;
    int status_ = 0;
    int note_ = kNoNoteYet;
};

void writeAcross(sample::Screen& screen, const char* words, int y, sample::Colour colour, int size) {
    screen.write(words, (sample::kScreenWidth - sample::Screen::widthOf(words, size)) / 2, y, colour, size);
}

const sample::Screen& waitingPicture() {
    static sample::Screen picture;
    picture.clear(kDark);
    picture.fill(0, 0, sample::kScreenWidth, 6, kAccent);
    writeAcross(picture, "MP FORGE LINK", 56, kInk, 4);
    writeAcross(picture, "PLUG THE USB-C CABLE INTO THE COMPUTER", 112, kSoft, 2);
    writeAcross(picture, "AND OPEN AN APP IN MP FORGE", 130, kSoft, 2);
    writeAcross(picture, "PUSH THE DIAL TO LEAVE", 196, kInk, 2);
    return picture;
}

// The carrying itself: for one computer after another, between it and the device's screen and controls.
class Link {
public:
    Link(Panel panel, int controls, int door) : panel_(panel), controls_(controls), door_(door) {}

    // Comes back when the dial is pushed while no computer is linked.
    void run() {
        for (;;) {
            panel_.show(waitingPicture());
            const int computer = nextComputer();
            if (computer == kNobody) return;
            carryFor(computer);
            close(computer);
        }
    }

private:
    int nextComputer() {
        DialPush dialPush;
        for (;;) {
            pollfd watched[2] = {{door_, POLLIN, 0}, {controls_, POLLIN, 0}};
            if (!waitFor(watched)) return kNobody;
            if (watched[1].revents != 0 && isLeftBy(dialPush)) return kNobody;
            if ((watched[0].revents & POLLIN) == 0) continue;
            const int computer = accept(door_, nullptr, nullptr);
            if (computer >= 0) return computer;
        }
    }

    static bool waitFor(pollfd (&watched)[2]) {
        while (poll(watched, 2, -1) < 0)
            if (errno != EINTR) return false;
        return true;
    }

    // Controls that can no longer be read end the program too, so that the device is not left stuck.
    bool isLeftBy(DialPush& dialPush) {
        uint8_t sent[256];
        const ssize_t count = read(controls_, sent, sizeof sent);
        if (count < 0 && errno == EINTR) return false;
        return count <= 0 || dialPush.isIn(sent, static_cast<size_t>(count));
    }

    void carryFor(int computer) {
        if (!greet(computer)) return;
        for (;;) {
            pollfd watched[2] = {{computer, POLLIN, 0}, {controls_, POLLIN, 0}};
            if (!waitFor(watched)) return;
            if (watched[1].revents != 0 && !handOnTheControlsTo(computer)) return;
            if (watched[0].revents != 0 && !takeAMessageFrom(computer)) return;
        }
    }

    // A computer that stops in the middle of a message is let go, so the controls are never held up for long.
    static bool greet(int computer) {
        const int atOnce = 1;
        const timeval patience{kSecondsAComputerMayTakeOverAMessage, 0};
        setsockopt(computer, IPPROTO_TCP, TCP_NODELAY, &atOnce, sizeof atOnce);
        setsockopt(computer, SOL_SOCKET, SO_RCVTIMEO, &patience, sizeof patience);
        return writeAll(computer, reinterpret_cast<const uint8_t*>(kGreeting), sizeof kGreeting - 1);
    }

    bool handOnTheControlsTo(int computer) {
        uint8_t sent[256];
        const ssize_t count = read(controls_, sent, sizeof sent);
        if (count < 0 && errno == EINTR) return true;
        return count > 0 && writeAll(computer, sent, static_cast<size_t>(count));
    }

    bool takeAMessageFrom(int computer) {
        uint8_t kind = 0;
        if (!readAll(computer, &kind, 1)) return false;
        if (kind == kRowsForTheScreen) return showRowsFrom(computer);
        if (kind == kBytesForTheControls) return passOnToTheControlsFrom(computer);
        return false;
    }

    bool showRowsFrom(int computer) {
        uint8_t which[4];
        if (!readAll(computer, which, sizeof which)) return false;
        const int firstRow = numberAt(which), rowCount = numberAt(which + 2);
        if (rowCount == 0 || firstRow + rowCount > sample::kScreenHeight) return false;
        taken_.resize(static_cast<size_t>(rowCount) * kBytesARow);
        if (!readAll(computer, taken_.data(), taken_.size())) return false;
        panel_.showRows(firstRow, taken_.data(), taken_.size());
        return true;
    }

    bool passOnToTheControlsFrom(int computer) {
        uint8_t howMany[2];
        if (!readAll(computer, howMany, sizeof howMany)) return false;
        const size_t byteCount = static_cast<size_t>(numberAt(howMany));
        if (byteCount > kMostBytesForTheControls) return false;
        taken_.resize(byteCount);
        return readAll(computer, taken_.data(), byteCount) && writeAll(controls_, taken_.data(), byteCount);
    }

    Panel panel_;
    int controls_;
    int door_;
    std::vector<uint8_t> taken_;
};

// The door computers come in by: a socket waiting at the port, on whichever network the device is on.
int doorAt(int port) {
    const int door = socket(AF_INET, SOCK_STREAM, 0);
    const int again = 1;
    sockaddr_in here{};
    here.sin_family = AF_INET;
    here.sin_addr.s_addr = htonl(INADDR_ANY);
    here.sin_port = htons(static_cast<uint16_t>(port));
    setsockopt(door, SOL_SOCKET, SO_REUSEADDR, &again, sizeof again);
    const bool isOpen = door >= 0 && bind(door, reinterpret_cast<const sockaddr*>(&here), sizeof here) == 0 && listen(door, 1) == 0;
    return isOpen ? door : kNobody;
}

int couldNotStart(const char* why, const char* what) {
    std::fprintf(stderr, "mp-forge-link: %s %s\n", why, what);
    return EXIT_FAILURE;
}

}  // namespace

int main(int argumentCount, char** arguments) {
    if (argumentCount != 4) return couldNotStart("it is started as", "mp-forge-link <screen> <controls> <port>");
    const int screen = open(arguments[1], O_WRONLY);
    if (screen < 0) return couldNotStart("the screen could not be opened:", arguments[1]);
    const int controls = open(arguments[2], O_RDWR);
    if (controls < 0) return couldNotStart("the controls could not be opened:", arguments[2]);
    const int door = doorAt(std::atoi(arguments[3]));
    if (door == kNobody) return couldNotStart("nothing could wait at port", arguments[3]);
    signal(SIGPIPE, SIG_IGN);
    Link(Panel(screen), controls, door).run();
    return EXIT_SUCCESS;
}
