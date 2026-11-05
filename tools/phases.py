#!/usr/bin/env python
"""Where the phases of a track are, so it can be cut into scenes.

    python tools/phases.py static/music/sine.mp3                 # detect tempo, print the map
    python tools/phases.py static/music/sine.mp3 --bpm 135       # trust this tempo
    python tools/phases.py static/music/sine.mp3 --bpm 135 --sections 1,17,33,49,65,73
                                                                 # cut here (1-based bars): prints Rust

Prints: the tempo (scanned, or the one given), how many bars the file is, a per-4-bar
energy map (overall / low / mid / high, dB relative to the loudest block) and the places
the band balance jumps — section boundaries, usually. Bars are 1-based in the printout
and taken from t = 0 (Ableton bounces start on a bar line); `--offset` moves the grid.

Needs numpy and miniaudio:  pip install --user numpy miniaudio
"""
import argparse
import sys

import numpy as np
import miniaudio

SR = 22050
HOP = 256
FPS = SR / HOP


def load(path):
    d = miniaudio.decode_file(path, output_format=miniaudio.SampleFormat.FLOAT32, nchannels=1, sample_rate=SR)
    return np.frombuffer(d.samples, dtype=np.float32)


def spectrogram(y, n_fft=1024):
    win = np.hanning(n_fft).astype(np.float32)
    n = 1 + (len(y) - n_fft) // HOP
    idx = np.arange(n_fft)[None, :] + HOP * np.arange(n)[:, None]
    return np.abs(np.fft.rfft(y[idx] * win, axis=1)), np.fft.rfftfreq(n_fft, 1 / SR)


def onset_strength(mag):
    flux = np.diff(np.log1p(mag * 100), axis=0)
    out = np.maximum(flux, 0).sum(axis=1)
    return out / out.mean()


def grid_fit(onset, bpm):
    """How much stronger than chance the onsets are on this tempo's beat grid (best phase)."""
    beat = 60 / bpm * FPS
    n = int((len(onset) - beat) / beat)
    best = 0.0
    for phase in np.arange(0, beat, 0.5):
        idx = np.round(phase + beat * np.arange(n)).astype(int)
        best = max(best, onset[idx].mean())
    return best


def scan_tempo(onset, lo, hi):
    # coarse autocorrelation first, then a fine fit around the peaks
    o = onset - onset.mean()
    ac = np.correlate(o, o, mode='full')[len(o) - 1:]
    cands = sorted(
        ((ac[lag], 60 * FPS / lag) for lag in range(int(FPS * 60 / hi), int(FPS * 60 / lo))),
        reverse=True,
    )
    peaks = []
    for _, bpm in cands:
        if all(abs(bpm - p) > 3 for p in peaks):
            peaks.append(bpm)
        if len(peaks) == 6:
            break
    fits = []
    for peak in peaks:
        for bpm in np.arange(peak - 2, peak + 2, 0.1):
            if lo <= bpm <= hi:
                fits.append((grid_fit(onset, bpm), round(bpm, 1)))
    fits.sort(reverse=True)
    return fits


def energy_map(mag, freqs, bar_s, offset, dur):
    lo, mid, hi = freqs < 150, (freqs >= 150) & (freqs < 2000), freqs >= 2000
    nbars = int((dur - offset) / bar_s)
    rows = []
    for b in range(nbars):
        a, z = int((offset + b * bar_s) * FPS), int((offset + (b + 1) * bar_s) * FPS)
        p = (mag[a:z] ** 2).mean(axis=0)
        rows.append([10 * np.log10(x.sum() + 1e-12) for x in (p, p[lo], p[mid], p[hi])])
    return np.array(rows)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('path')
    ap.add_argument('--bpm', type=float, help='tempo to trust (else scanned)')
    ap.add_argument('--range', default='70,180', help='tempo scan range, lo,hi')
    ap.add_argument('--offset', type=float, default=0.0, help='seconds into the file where bar 1 starts')
    ap.add_argument('--block', type=int, default=4, help='bars per row of the map')
    ap.add_argument('--sections', help='1-based bars where sections start, comma-separated: prints Rust')
    ap.add_argument('--id', help='track id for the Rust (default: file stem)')
    args = ap.parse_args()

    y = load(args.path)
    dur = len(y) / SR
    mag, freqs = spectrogram(y)
    onset = onset_strength(mag)

    bpm = args.bpm
    if bpm is None:
        lo, hi = (float(x) for x in args.range.split(','))
        fits = scan_tempo(onset, lo, hi)
        print('tempo (beat-grid fit, 1.0 = chance): ' + ', '.join(f'{b} ({f:.1f})' for f, b in fits[:6]))
        bpm = fits[0][1]
        print(f'taking {bpm}; pass --bpm to override')
    bar_s = 240 / bpm
    nbars = (dur - args.offset) / bar_s
    db = 20 * np.log10(np.abs(y) + 1e-9)
    first = np.argmax(db > -40) / SR
    print(f'{args.path}: {dur:.2f} s at {bpm} bpm = {nbars:.2f} bars ({bar_s:.4f} s/bar), '
          f'sound from {first:.2f} s, grid fit {grid_fit(onset, bpm):.1f}')

    rows = energy_map(mag, freqs, bar_s, args.offset, dur)
    ref = rows[:, 0].max()
    rel = rows - ref
    print(f'bars        total    low    mid   high   (dB rel. loudest {args.block}-bar block)')
    prev = None
    for b in range(0, len(rows), args.block):
        blk = rel[b:b + args.block].mean(axis=0)
        jump = ''
        if prev is not None:
            d = blk[1:] - prev[1:]
            if np.abs(d).max() > 6:
                jump = f'  <- L{d[0]:+.0f} M{d[1]:+.0f} H{d[2]:+.0f}'
        bar = '#' * int(max(0.0, 40 + blk[0]) / 1.5)
        print(f'{b + 1:4}-{min(b + args.block, len(rows)):<4} {blk[0]:6.1f} {blk[1]:6.1f} {blk[2]:6.1f} {blk[3]:6.1f}  {bar}{jump}')
        prev = blk

    if args.sections:
        starts = [int(s) for s in args.sections.split(',')]
        ends = starts[1:] + [int(nbars)]
        tid = args.id or args.path.rsplit('/', 1)[-1].rsplit('\\', 1)[-1].rsplit('.', 1)[0]
        const = tid.upper().replace('-', '_').replace('.', '_')
        print()
        print(f'pub const {const}: Track = Track {{')
        print(f'\tid: "{tid}",')
        print(f'\tpath: "{args.path.split("static/", 1)[-1]}",')
        print(f'\tbpm: {bpm},')
        print('};')
        print('// sections, 0-based bar ranges (end exclusive):')
        for k, (a, z) in enumerate(zip(starts, ends)):
            blk = rel[a - 1:z - 1].mean(axis=0) if z > a else rel[a - 1]
            print(f'//   {k}: bars {a - 1}..{z - 1}  ({z - a} bars, {blk[0]:.0f} dB, L{blk[1]:.0f} M{blk[2]:.0f} H{blk[3]:.0f})')


if __name__ == '__main__':
    main()
