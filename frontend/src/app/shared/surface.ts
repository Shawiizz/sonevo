import { Application, Container, Graphics, Text, TextStyle } from 'pixi.js';

/** A colour as `#rrggbb`, as Pixi takes it. */
export function hex(color: string): number {
  return Number.parseInt(color.replace('#', ''), 16) || 0;
}

/** The CSS custom property `name` of the page, as a colour. */
export function token(name: string): number {
  return hex(getComputedStyle(document.documentElement).getPropertyValue(name).trim());
}

const STYLE = new TextStyle({
  fontFamily: 'Geist Variable, Segoe UI, sans-serif',
  fontSize: 11,
  fill: 0xbcc2cf,
});

/**
 * A WebGL canvas filling `host`, drawn again only when asked: shapes into `shapes`, words
 * from a pool that grows as they are needed.
 */
export class Surface {
  readonly app = new Application();
  readonly shapes = new Graphics();
  /** Drawn over the shapes: the playhead, a selection being made. */
  readonly over = new Graphics();
  private readonly words = new Container();
  private readonly pool: Text[] = [];
  private used = 0;
  private wanted = true;
  private ready = false;
  private destroyed = false;
  private observer: ResizeObserver | null = null;
  width = 0;
  height = 0;

  constructor(
    private readonly host: HTMLElement,
    private readonly paint: (surface: Surface) => void,
  ) {}

  async start(): Promise<void> {
    await this.app.init({
      resizeTo: this.host,
      backgroundAlpha: 0,
      antialias: true,
      resolution: window.devicePixelRatio || 1,
      autoDensity: true,
      preference: 'webgl',
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
    this.app.ticker.add(() => {
      if (!this.wanted) return;
      this.wanted = false;
      this.width = this.host.clientWidth;
      this.height = this.host.clientHeight;
      this.shapes.clear();
      this.over.clear();
      this.used = 0;
      this.paint(this);
      for (let i = this.used; i < this.pool.length; i++) this.pool[i].visible = false;
    });
    this.ready = true;
  }

  redraw(): void {
    this.wanted = true;
  }

  /** Words at (x, y), their top left there unless `anchor` says otherwise. */
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
