// The file Linux keeps a screen's picture in, as the "Screen Probe" test card reads and paints it.
//
// SPDX-License-Identifier: MIT
//
// On Linux a screen may be offered as a file (/dev/fb0): its bytes are the dots on the screen, row after row.
// Whether an MPC offers one, and whether what is written to it is what the screen shows, is what the card is
// there to find out. Nothing here changes how the screen is set up: it asks, reads and writes dots, no more.

#pragma once

#include <algorithm>
#include <cstdint>
#include <cstring>
#include <fcntl.h>
#include <unistd.h>
#include <vector>

#if defined(__linux__)
#include <linux/fb.h>
#include <sys/ioctl.h>
#endif

namespace screen_test {

struct Colour { uint8_t red, green, blue; };

// A part of the screen, in dots from its top-left corner.
struct Block { int left, top, across, down; };

// Where one part of a colour sits among a dot's bits.
struct ColourPart
{
    int firstBit = 0;
    int bits     = 0;

    uint32_t placed(uint8_t strength) const
    {
        const int kept = std::min(bits, kBitsOfAStrength);
        return (uint32_t)(strength >> (kBitsOfAStrength - kept)) << (firstBit + bits - kept);
    }

    static constexpr int kBitsOfAStrength = 8;
};

// How a screen's dots lie in its file.
struct ScreenLayout
{
    static constexpr int kNameLength = 16;

    int        across = 0, down = 0;
    int        bytesADot = 0, bytesARow = 0;
    long       firstByteShown = 0;
    ColourPart red, green, blue;
    char       name[kNameLength + 1] = {};

    bool holds(const Block& block) const
    {
        const bool isKnown = bytesADot >= 2 && bytesADot <= 4 && bytesARow >= across * bytesADot;
        const bool isInside = block.left >= 0 && block.top >= 0 && block.across > 0 && block.down > 0
                           && block.left + block.across <= across && block.top + block.down <= down;
        return isKnown && isInside;
    }

    long firstByteOf(const Block& block, int row) const
    {
        return firstByteShown + (long)(block.top + row) * bytesARow + (long)block.left * bytesADot;
    }

    size_t bytesOf(const Block& block) const { return (size_t)block.across * (size_t)block.down * (size_t)bytesADot; }

    uint32_t dotOf(Colour colour) const { return red.placed(colour.red) | green.placed(colour.green) | blue.placed(colour.blue); }
};

#if defined(__linux__)
// False when the file is not a screen that answers.
inline bool askedOf(int file, ScreenLayout& layout)
{
    fb_var_screeninfo asSet {};
    fb_fix_screeninfo asBuilt {};
    if (ioctl(file, FBIOGET_VSCREENINFO, &asSet) != 0 || ioctl(file, FBIOGET_FSCREENINFO, &asBuilt) != 0)
        return false;
    layout.across         = (int)asSet.xres;
    layout.down           = (int)asSet.yres;
    layout.bytesADot      = (int)asSet.bits_per_pixel / 8;
    layout.bytesARow      = (int)asBuilt.line_length;
    layout.firstByteShown = (long)asSet.yoffset * layout.bytesARow + (long)asSet.xoffset * layout.bytesADot;
    layout.red            = { (int)asSet.red.offset, (int)asSet.red.length };
    layout.green          = { (int)asSet.green.offset, (int)asSet.green.length };
    layout.blue           = { (int)asSet.blue.offset, (int)asSet.blue.length };
    std::memcpy(layout.name, asBuilt.id, ScreenLayout::kNameLength);
    return true;
}
#else
// Only Linux offers a screen this way. Elsewhere the card says so and paints nothing.
inline bool askedOf(int, ScreenLayout&) { return false; }
#endif

class ScreenFile
{
public:
    enum HowFar { kWaiting = 0, kNoFile, kCannotOpen, kCannotAsk, kOpen };

    ScreenFile() = default;
    ScreenFile(const ScreenFile&) = delete;
    ScreenFile& operator=(const ScreenFile&) = delete;
    ~ScreenFile() { if (file_ >= 0) close(file_); }

    HowFar openAt(const char* path)
    {
        if (!openTheFileAt(path))
            return howFar_;
        return howFar_ = askedOf(file_, layout_) ? kOpen : kCannotAsk;
    }

    // For a file that is not a screen: it is taken to lie the way it is told.
    HowFar openAt(const char* path, const ScreenLayout& layout)
    {
        if (!openTheFileAt(path))
            return howFar_;
        layout_ = layout;
        return howFar_ = kOpen;
    }

    HowFar              howFar() const { return howFar_; }
    const ScreenLayout& layout() const { return layout_; }
    bool                holds(const Block& block) const { return howFar_ == kOpen && layout_.holds(block); }

    bool read(const Block& block, std::vector<uint8_t>& dots) const
    {
        dots.resize(layout_.bytesOf(block));
        return passesEveryRow(block, [&](int row, size_t byteCount, long at) {
            return pread(file_, dots.data() + placeOfRow(block, row), byteCount, (off_t)at) == (ssize_t)byteCount;
        });
    }

    bool write(const Block& block, const std::vector<uint8_t>& dots) const
    {
        if (dots.size() != layout_.bytesOf(block))
            return false;
        return passesEveryRow(block, [&](int row, size_t byteCount, long at) {
            return pwrite(file_, dots.data() + placeOfRow(block, row), byteCount, (off_t)at) == (ssize_t)byteCount;
        });
    }

private:
    bool openTheFileAt(const char* path)
    {
        if (access(path, F_OK) != 0)
            return leftAt(kNoFile);
        file_ = open(path, O_RDWR);
        return file_ >= 0 || leftAt(kCannotOpen);
    }

    bool leftAt(HowFar howFar)
    {
        howFar_ = howFar;
        return false;
    }

    // Where a row of a block starts among the block's own bytes.
    size_t placeOfRow(const Block& block, int row) const
    {
        return (size_t)row * (size_t)block.across * (size_t)layout_.bytesADot;
    }

    template <typename PassARow>
    bool passesEveryRow(const Block& block, PassARow passARow) const
    {
        if (!holds(block))
            return false;
        const size_t bytesInARow = (size_t)block.across * (size_t)layout_.bytesADot;
        for (int row = 0; row < block.down; ++row)
            if (!passARow(row, bytesInARow, layout_.firstByteOf(block, row)))
                return false;
        return true;
    }

    int          file_   = -1;
    HowFar       howFar_ = kWaiting;
    ScreenLayout layout_;
};

} // namespace screen_test
