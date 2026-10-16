import { fuzzy } from './palette';

describe('palette', () => {
  it('finds commands by a few of their letters, in order', () => {
    expect(fuzzy('sav', 'Save as…')).not.toBeNull();
    expect(fuzzy('xyz', 'Save as…')).toBeNull();
    expect(fuzzy('vas', 'Save')).toBeNull();
    // the start of a word counts more
    expect(fuzzy('pr', 'Piano roll')!).toBeGreaterThan(fuzzy('pr', 'Open recent')!);
  });
});
