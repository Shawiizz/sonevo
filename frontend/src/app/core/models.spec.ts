import {
  barLength,
  chordKeys,
  degreeChord,
  formatPosition,
  isBlack,
  keyName,
  newId,
  PPQ,
  sampleLabel,
  SCALES,
} from './models';

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

  it('makes chords of a scale, or major without one', () => {
    expect(chordKeys(60, 3, 0, null)).toEqual([60, 64, 67]);
    expect(chordKeys(60, 4, 0, null)).toEqual([60, 64, 67, 70]);
    const major = [0, 2, 4, 5, 7, 9, 11];
    // in C major, D's chord is minor, G's a dominant seventh
    expect(chordKeys(62, 3, 0, major)).toEqual([62, 65, 69]);
    expect(chordKeys(67, 4, 0, major)).toEqual([67, 71, 74, 77]);
    // a key out of the scale stays alone
    expect(chordKeys(61, 3, 0, major)).toEqual([61]);
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

  it('builds the chords of a scale on its degrees', () => {
    const [major, minor] = [SCALES[0].steps, SCALES[1].steps];
    expect(degreeChord(0, major, 0, 48)).toEqual([48, 52, 55]);
    expect(degreeChord(5, major, 0, 48)).toEqual([57, 60, 64]);
    expect(degreeChord(6, minor, 0, 48)).toEqual([58, 62, 65]);
  });
});
