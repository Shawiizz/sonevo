import { barLength, formatPosition, isBlack, keyName, newId, PPQ, sampleLabel } from './models';

describe('models', () => {
  it('writes positions as bars, beats and steps from 1', () => {
    const four = { beats: 4, unit: 4 };
    expect(formatPosition(0, four)).toBe('1.1.1');
    expect(formatPosition(PPQ, four)).toBe('1.2.1');
    expect(formatPosition(PPQ * 4 + PPQ / 4, four)).toBe('2.1.2');
    expect(formatPosition(-10, four)).toBe('1.1.1');
    const waltz = { beats: 3, unit: 4 };
    expect(formatPosition(PPQ * 3, waltz)).toBe('2.1.1');
  });

  it('knows a bar by its signature', () => {
    expect(barLength({ beats: 4, unit: 4 })).toBe(PPQ * 4);
    expect(barLength({ beats: 6, unit: 8 })).toBe(PPQ * 3);
  });

  it('names keys, middle C being C4', () => {
    expect(keyName(60)).toBe('C4');
    expect(keyName(69)).toBe('A4');
    expect(keyName(61)).toBe('C#4');
    expect(isBlack(61)).toBe(true);
    expect(isBlack(64)).toBe(false);
  });

  it('names sounds by their own name', () => {
    expect(sampleLabel('Kick 808 0123456789abcdef.wav')).toBe('Kick 808');
    expect(sampleLabel('0123456789abcdef.flac')).toBe('0123456789abcdef.flac');
  });

  it('makes ids as the song has them', () => {
    const ids = new Set(Array.from({ length: 100 }, newId));
    expect(ids.size).toBe(100);
    for (const id of ids) expect(id).toMatch(/^[0-9a-f]{16}$/);
  });
});
