import { Application, Container, Graphics, Text, TextStyle } from 'pixi.js';

export function hex(color: string): number {
  return Number.parseInt(color.replace('#', ''), 16) || 0;
}

export function token(name: string): number {
  return hex(getComputedStyle(document.documentElement).getPropertyValue(name).trim());
}

const DARK = {
  accent: 0x8f8bf7,
  play: 0x5fd3a8,
  base: 0x0f1115,
  void: 0x0c0e12,
  line: 0x252a33,
  lineStrong: 0x363c48,
  text: 0xf0f2f6,
  text2: 0xbcc2cf,
  text3: 0x8d95a5,
  /** Words on a colour of a clip or a note. */
  onColor: 0x0b0c10,
  picked: 0xffffff,
  /** What dims a part: past a pattern's end, a muted track. */
  shade: 0x000000,
  rowWhite: 0x15181e,
  rowBlack: 0x111318,
  gridOctave: 0x2c323d,
  gridBar: 0x3a414f,
  gridBeat: 0x252a33,
  gridStep: 0x1a1e25,
  scaleRoot: 0x24213f,
  scaleIn: 0x1b1e27,
  scaleOut: 0x0e1014,
  keyWhite: 0xd7dbe3,
  keyBlack: 0x0b0c10,
  keyLabel: 0x4a5160,
  laneA: 0x12151a,
  laneB: 0x101217,
  laneLine: 0x1c2028,
};

const LIGHT: typeof DARK = {
  accent: 0x6d5ce8,
  play: 0x12936a,
  base: 0xf7f8fa,
  void: 0xeceff3,
  line: 0xd8dce4,
  lineStrong: 0xc2c8d3,
  text: 0x14161c,
  text2: 0x3b4250,
  text3: 0x69717f,
  onColor: 0x0b0c10,
  picked: 0x14161c,
  shade: 0xffffff,
  rowWhite: 0xf6f7f9,
  rowBlack: 0xebeef2,
  gridOctave: 0xc2c8d3,
  gridBar: 0xb3bac6,
  gridBeat: 0xd8dce4,
  gridStep: 0xe8ebf0,
  scaleRoot: 0xdcd6fb,
  scaleIn: 0xf6f7f9,
  scaleOut: 0xe0e3e9,
  keyWhite: 0xffffff,
  keyBlack: 0x22252d,
  keyLabel: 0x8a91a0,
  laneA: 0xf7f8fa,
  laneB: 0xf1f3f6,
  laneLine: 0xe2e5eb,
};

export function palette(): typeof DARK {
  return document.documentElement.dataset['theme'] === 'light' ? LIGHT : DARK;
}

const STYLE = new TextStyle({
  fontFamily: 'Geist Variable, Segoe UI, sans-serif',
  fontSize: 11,
  fill: 0xbcc2cf,
});

/**
 * A WebGL canvas filling `host`, drawn again only when asked; what moves on its own (the
 * playhead) is drawn over it, alone.
 */
export class Surface {
  readonly app = new Application();
  readonly shapes = new Graphics();
  readonly over = new Graphics();
  private readonly words = new Container();
  private readonly pool: Text[] = [];
  private used = 0;
  private wanted = true;
  private wantedOver = true;
  private ready = false;
  private destroyed = false;
  private observer: ResizeObserver | null = null;
  width = 0;
  height = 0;

  constructor(
    private readonly host: HTMLElement,
    private readonly paint: (surface: Surface) => void,
    private readonly paintOver: (surface: Surface) => void = () => undefined,
  ) {}

  async start(): Promise<void> {
    await this.app.init({
      backgroundAlpha: 0,
      antialias: true,
      resolution: window.devicePixelRatio || 1,
      autoDensity: true,
      preference: 'webgl',
      // drawn when something changed, never otherwise
      autoStart: false,
    });
    if (this.destroyed) {
      this.app.destroy(true);
      return;
    }
    this.host.appendChild(this.app.canvas);
    this.app.canvas.style.display = 'block';
    this.app.stage.addChild(this.shapes, this.words, this.over);
    this.observer = new ResizeObserver(() => this.redraw());
    this.observer.observe(this.host);
    this.ready = true;
    this.frame();
  }

  /** Draws what is wanted, once a frame at most, and the canvas only if anything was. */
  private frame(): void {
    if (this.destroyed) return;
    const drawn = this.wanted || this.wantedOver;
    if (this.wanted) {
      this.wanted = false;
      this.wantedOver = true;
      this.width = this.host.clientWidth;
      this.height = this.host.clientHeight;
      this.app.renderer.resize(this.width, this.height);
      this.shapes.clear();
      this.used = 0;
      this.paint(this);
      for (let i = this.used; i < this.pool.length; i++) this.pool[i].visible = false;
    }
    if (this.wantedOver) {
      this.wantedOver = false;
      this.over.clear();
      this.paintOver(this);
    }
    if (drawn) this.app.render();
    requestAnimationFrame(() => this.frame());
  }

  redraw(): void {
    this.wanted = true;
  }

  redrawOver(): void {
    this.wantedOver = true;
  }

  text(words: string, x: number, y: number, color = 0xbcc2cf, size = 11, anchor = 0): void {
    let t = this.pool[this.used];
    if (!t) {
      t = new Text({ text: words, style: STYLE.clone() });
      this.pool.push(t);
      this.words.addChild(t);
    }
    this.used++;
    if (t.text !== words) t.text = words;
    if (t.style.fill !== color) t.style.fill = color;
    if (t.style.fontSize !== size) t.style.fontSize = size;
    t.anchor.set(anchor, 0);
    t.position.set(Math.round(x), Math.round(y));
    t.visible = true;
  }

  destroy(): void {
    this.destroyed = true;
    this.observer?.disconnect();
    if (this.ready) this.app.destroy(true, { children: true });
  }
}
