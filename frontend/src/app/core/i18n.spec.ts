import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { FR } from './fr';

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
  // the names of the devices and their settings, as the engine gives them
  const engine = read(sources(join(__dirname, '../../../../crates/sonevo-dsp/src'), '.rs'));
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
