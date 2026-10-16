import { formatValue, fromNormal, toNormal } from './knob';

describe('knob', () => {
  it('goes to and from the knob, straight or by steps that multiply', () => {
    for (const log of [false, true]) {
      for (const p of [0, 0.25, 0.5, 1]) {
        expect(toNormal(fromNormal(p, 20, 20000, log), 20, 20000, log)).toBeCloseTo(p, 9);
      }
    }
    expect(fromNormal(0.5, 20, 20000, true)).toBeCloseTo(632.46, 1);
    expect(fromNormal(0.5, -1, 1, false)).toBe(0);
    expect(toNormal(99999, 0, 1, false)).toBe(1);
  });

  it('shows values with their units', () => {
    expect(formatValue(0.005, 's')).toBe('5 ms');
    expect(formatValue(1.5, 's')).toBe('1.50 s');
    expect(formatValue(440, 'Hz')).toBe('440 Hz');
    expect(formatValue(2500, 'Hz')).toBe('2.50 kHz');
    expect(formatValue(0.4, 'Hz')).toBe('0.40 Hz');
    expect(formatValue(-6, 'dB')).toBe('-6.00 dB');
    expect(formatValue(0.4, '')).toBe('0.40');
  });
});
