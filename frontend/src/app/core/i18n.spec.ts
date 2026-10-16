import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { FR } from './fr';
import { I18n } from './i18n';

/** Every source file under `dir` ending in `extension`, but the tests. */
function sources(dir: string, extension: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return sources(path, extension);
    return name.endsWith(extension) && !name.endsWith('.spec.ts') && name !== 'fr.ts' ? [path] : [];
  });
}

const read = (paths: string[]) => paths.map((p) => readFileSync(p, 'utf8')).join('\n');

describe('French', () => {
  const code = read(sources(join(__dirname, '..'), '.ts'));
  // the names the Rust side gives: devices and their settings, versions made by themselves
  const engine = read(
    ['crates/sonevo-dsp/src', 'crates/sonevo-versions/src', 'apps/sonevo-desktop/src'].flatMap(
      (dir) => sources(join(__dirname, '../../../..', dir), '.rs'),
    ),
  );
  const said = [...code.matchAll(/\bt\(\s*'((?:[^'\\]|\\.)*)'/g)].map((m) =>
    m[1].replace(/\\'/g, "'"),
  );

  it('has every text the interface says', () => {
    const missing = [...new Set(said)].filter((text) => !(text in FR));
    expect(missing).toEqual([]);
  });

  it('has no text the interface no longer says', () => {
    const unused = Object.keys(FR).filter(
      (text) => !code.includes(`'${text.replace(/'/g, "\\'")}'`) && !engine.includes(`"${text}"`),
    );
    expect(unused).toEqual([]);
  });
});

describe('Names Sonevo gives', () => {
  it('read in the language of the interface, and only those', () => {
    const i18n = new I18n();
    i18n.set('fr');
    expect(i18n.named('Track 12')).toBe('Piste 12');
    expect(i18n.named('Untitled')).toBe('Sans titre');
    expect(i18n.named('Track of my life')).toBe('Track of my life');
    i18n.set('en');
    expect(i18n.named('Track 12')).toBe('Track 12');
  });
});
