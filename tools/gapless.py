#!/usr/bin/env python
"""Give an mp3 the gapless header its encoder left out.

    python tools/gapless.py static/music/sine.mp3 [more.mp3 ...]      # in place
    python tools/gapless.py --delay 1729 ...                           # encoder delay, in samples

Ableton's mp3 export writes no Xing/LAME "Info" frame, so decoders can't know how much
silence the encoder put in front of the music (here 2258 samples = 51 ms, measured
against a wav of the same bounce: 1729 of encoder delay plus the 529 every decoder adds).
Without it, bar 0 is 51 ms into the decoded audio and every loop point and switch lands
51 ms early. This prepends a standard Info frame — same header as the audio frames, a
LAME tag carrying the encoder delay — and leaves every audio frame untouched. Browsers
(Chrome, Firefox, Safari) then trim the delay on decode, and t = 0 is the downbeat.

Idempotent: a file that already has an Info/Xing frame is left alone.
"""
import argparse
import struct
import sys

BITRATES = [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320]
RATES = {3: [44100, 48000, 32000], 2: [22050, 24000, 16000], 0: [11025, 12000, 8000]}


def header(b, i):
    """(frame length, side info size, is MPEG1) for the frame header at b[i:i+4], or None."""
    if i + 4 > len(b) or b[i] != 0xFF or (b[i + 1] & 0xE0) != 0xE0:
        return None
    version = (b[i + 1] >> 3) & 3
    layer = (b[i + 1] >> 1) & 3
    if version == 1 or layer != 1:  # reserved version / not layer III
        return None
    bitrate = BITRATES[b[i + 2] >> 4] if version == 3 else None
    if bitrate is None:
        # MPEG2/2.5 layer III table
        bitrate = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160][b[i + 2] >> 4]
    rate = RATES[version][(b[i + 2] >> 2) & 3] if ((b[i + 2] >> 2) & 3) != 3 else None
    if not bitrate or not rate:
        return None
    padding = (b[i + 2] >> 1) & 1
    mono = (b[i + 3] >> 6) == 3
    if version == 3:
        length = 144 * bitrate * 1000 // rate + padding
        side = 17 if mono else 32
    else:
        length = 72 * bitrate * 1000 // rate + padding
        side = 9 if mono else 17
    return length, side, version == 3


def crc16(data):
    """LAME's tag CRC: CRC-16/ARC (poly 0x8005 reflected), init 0."""
    crc = 0
    for byte in data:
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ 0xA001 if crc & 1 else crc >> 1
    return crc


def gapless(path, delay, padding=0):
    b = open(path, 'rb').read()
    start = 0
    if b[:3] == b'ID3':
        start = 10 + ((b[6] << 21) | (b[7] << 14) | (b[8] << 7) | b[9])
    i = start
    while header(b, i) is None:
        i += 1
        if i > len(b) - 4:
            raise SystemExit(f'{path}: no mp3 frame found')
    length, side, mpeg1 = header(b, i)
    tag = b[i + 4 + side:i + 4 + side + 4]
    if tag in (b'Xing', b'Info'):
        print(f'{path}: already has a {tag.decode()} frame, left alone')
        return
    # count the audio frames
    frames, j = 0, i
    while True:
        h = header(b, j)
        if h is None:
            break
        frames += 1
        j += h[0]
    audio = b[i:j]
    # the Info frame: the first audio frame's header with the padding bit cleared
    head = bytearray(b[i:i + 4])
    head[2] &= ~0x02
    info_length = header(head + b'\0' * 4, 0)[0]
    frame = bytearray(info_length)
    frame[0:4] = head
    o = 4 + side
    frame[o:o + 4] = b'Info'
    o += 4
    frame[o:o + 4] = struct.pack('>I', 0x0F)
    o += 4
    frame[o:o + 4] = struct.pack('>I', frames)
    o += 4
    frame[o:o + 4] = struct.pack('>I', info_length + len(audio))
    o += 4
    frame[o:o + 100] = bytes(k * 256 // 100 for k in range(100))
    o += 100
    frame[o:o + 4] = struct.pack('>I', 0)
    o += 4
    assert o == 4 + side + 120
    lame = o
    frame[lame:lame + 9] = b'LAME3.100'
    frame[lame + 9] = 0x01  # revision 0, CBR
    frame[lame + 20] = 255  # bitrate byte: 255 means >= 255 kbps
    frame[lame + 21] = delay >> 4
    frame[lame + 22] = ((delay & 0xF) << 4) | (padding >> 8)
    frame[lame + 23] = padding & 0xFF
    frame[lame + 28:lame + 32] = struct.pack('>I', len(audio))
    frame[lame + 34:lame + 36] = struct.pack('>H', crc16(frame[:lame + 34]))
    out = b[:start] + bytes(frame) + audio + b[j:]
    open(path, 'wb').write(out)
    print(f'{path}: {frames} frames, Info frame of {info_length} bytes added, delay {delay} + 529 samples trimmed on decode')


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('paths', nargs='+')
    ap.add_argument('--delay', type=int, default=1729, help='encoder delay in samples (the decoder adds 529)')
    args = ap.parse_args()
    for path in args.paths:
        gapless(path, args.delay)


if __name__ == '__main__':
    main()
