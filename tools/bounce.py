"""The bars bounce + pinwheel as six ordinary cells layers: prints the `bounce` group preset (`python tools/bounce.py`).

Plain pair (angle locked): even/odd columns, angle up/down, length = two position→curtain
chains on bar.phase every 4 (grow+retreat up, grow+retreat from the other end as a negative
length). Pinwheel set (needs the rotation unlock): four layers by column parity ×
row parity; angle = V + a sequence of quarter turns (6 chains on bar.phase every 8, kept
within 90° of the plain angle so the crossfade in is a swing, never a flip); length = eight
one-bar curtains (grow/retreat along the pointing of that bar), signed so opposite directions
crossfade through the dot.
"""
import json


def chain(source, depth, threshold, width, every=1, offset=0, curve=0, invert=False):
    return {'source': source, 'depth': depth, 'threshold': threshold, 'width': width, 'curve': curve,
            'invert': invert, 'on': True, 'ripple': 0, 'every': every, 'offset': offset}


def phase8(depth, threshold, width):
    return chain('bar.phase', depth, threshold, width, every=8)


def phase4(depth, threshold, width):
    return chain('bar.phase', depth, threshold, width, every=4)


STEP = 0.0002  # a step: the width of a sidechain that is a switch (1/8 bar phase × 0.0002 ≈ 3 ms at 120)


def turn_of(deg):
    """A field's `turn` (0 up, clockwise to 1) for a direction in degrees (0 right, ccw)."""
    return round(((90 - deg) / 360) % 1, 4)


def curtain(turn, depth, offset_chains):
    return {'source': 'position', 'depth': depth, 'threshold': 0, 'width': 1, 'curve': 1, 'invert': False, 'on': True,
            'ripple': 0, 'every': 1, 'offset': 0,
            'field': {'fn': 'curtain', 'params': {
                'offset': {'base': 5.5, 'chains': offset_chains},
                'edge': {'base': 1, 'chains': []}, 'turn': {'base': turn, 'chains': []},
                'x': {'base': 0, 'chains': []}, 'y': {'base': 0, 'chains': []}}}}


def grow8(bar, turn, depth):
    """Over bar `bar` of 8: the edge runs 5.5 → −5.5 along `turn` — the line grows from the root (and steps back at the next bar)."""
    return curtain(turn, depth, [phase8(-11, bar / 8, 1 / 8), phase8(11, (bar + 1) / 8, STEP)])


def retreat8(bar, turn, depth):
    """Over bar `bar` of 8: −5.5 → 5.5 — the line retreats into the root (a step down at the bar, a ramp back up)."""
    return curtain(turn, depth, [phase8(-11, bar / 8, STEP), phase8(11, bar / 8, 1 / 8)])


def row(base, chains=(), masks=(), op='set'):
    return {'op': op, 'base': base, 'chains': list(chains), 'masks': list(masks)}


def plain(a, rows=False):
    """Even (a=0) / odd (a=1) columns: the bounce with the angle locked (as the preset already had).
    `rows`: even/odd rows instead, the lines pointing right/left (the owner's "turning bars" map)."""
    V = (0 if a == 0 else 180) if rows else (90 if a == 0 else 270)
    lane, word = ('rows', 'rows') if rows else ('cols', 'columns')
    def pair(turn, depth, q):  # grow over quarter q, retreat over q+1, of the 4-bar cycle
        return curtain(turn, depth, [phase4(-11, q * 0.25, 0.25), phase4(11, (q + 1) * 0.25, 0.25)])
    return {'name': 'bounce ' + ('even' if a == 0 else 'odd') + ' ' + word,
            'region': {lane: [2, a]}, 'kind': 'cells', 'params': {
                'brightness': row(0.9), 'dots': row(0.667), 'lines': row(1), 'angle': row(V),
                'length': row(0, [pair(turn_of(V + 180), 1, 0), pair(turn_of(V), -1, 2)])}}


def pinwheel(a, b, rows=False):
    """Column parity a × row parity b: what those cells do once the angle is unlocked.
    `rows`: a is the row parity and b the column's; the plain state points right/left."""
    V = (0 if a == 0 else 180) if rows else (90 if a == 0 else 270)
    s = -90 if a == b else 90  # the way this segment turns
    H = V + s
    beat = 1 / 32  # one beat of the 8-bar phase
    angle = [phase8(s, 1 / 8, beat), phase8(-180, 3 / 8, STEP), phase8(s, 3 / 8, beat),
             phase8(s, 5 / 8, beat), phase8(-180, 6 / 8, STEP), phase8(s, 7 / 8, beat)]
    length = [
        grow8(0, turn_of(V + 180), 1),      # columns grow from their end
        retreat8(1, turn_of(H + 180), 1),   # turn a quarter; retreat as rows
        grow8(2, turn_of(H), -1),           # rows grow back from the other side
        retreat8(3, turn_of(V + 180), 1),   # turn again; retreat as columns into the first ends
        grow8(4, turn_of(V), -1),           # columns from the other end
        retreat8(5, turn_of(H), -1),        # turn; retreat as rows the other way
        grow8(6, turn_of(H + 180), -1),     # rows back
        retreat8(7, turn_of(V), -1),        # turn; retreat as columns
    ]
    lane, other = ('rows', 'cols') if rows else ('cols', 'rows')
    words = {'cols': 'columns', 'rows': 'rows'}
    return {'name': f"pinwheel {'even' if a == 0 else 'odd'} {words[lane]}, {'even' if b == 0 else 'odd'} {words[other]}",
            'region': {lane: [2, a], other: [2, b]}, 'kind': 'cells', 'needs': ['rotation'], 'params': {
                'angle': row(V, angle),
                'length': row(0, length)}}


def layers(rows=False):
    return [plain(0, rows), plain(1, rows), pinwheel(0, 0, rows), pinwheel(0, 1, rows), pinwheel(1, 0, rows), pinwheel(1, 1, rows)]


if __name__ == '__main__':
    import sys
    print(json.dumps(layers(rows='--rows' in sys.argv), indent=1))
