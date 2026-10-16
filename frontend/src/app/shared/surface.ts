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

/** The palettes in use: the accent chosen in them. */
const shown = { dark: DARK, light: LIGHT };

export function setAccent(dark: number | null, light: number | null): void {
  shown.dark = dark === null ? DARK : { ...DARK, accent: dark };
  shown.light = light === null ? LIGHT : { ...LIGHT, accent: light };
}

export function palette(): typeof DARK {
  return document.documentElement.dataset['theme'] === 'light' ? shown.light : shown.dark;
}

/** A colour as a number (0xRRGGBB), or with how opaque it is. */
type Paint = number | { color: number; alpha?: number };

const CSS = new Map<number, string>();

function css(paint: Paint): string {
  const { color, alpha = 1 } = typeof paint === 'number' ? { color: paint } : paint;
  let rgb = CSS.get(color);
  if (!rgb) {
    rgb = `${(color >> 16) & 255}, ${(color >> 8) & 255}, ${color & 255}`;
    CSS.set(color, rgb);
  }
  return `rgba(${rgb}, ${alpha})`;
}

const FONT = 'Geist Variable, Segoe UI, sans-serif';

/** Draws on a canvas a shape at a time, filled or outlined once made. */
export class Pen {
  /** The screen's pixels in one of the canvas's own, to put edges on them. */
  ratio = 1;

  constructor(readonly context: CanvasRenderingContext2D) {}

  /** A rectangle, its sides on the screen's pixels so that a thin line stays sharp. */
  rect(x: number, y: number, width: number, height: number): this {
    const r = this.ratio;
    const left = Math.round(x * r) / r;
    const top = Math.round(y * r) / r;
    const w = Math.max(width > 0 ? 1 / r : 0, Math.round((x + width) * r) / r - left);
    const h = Math.max(height > 0 ? 1 / r : 0, Math.round((y + height) * r) / r - top);
    this.context.beginPath();
    this.context.rect(left, top, w, h);
    return this;
  }

  roundRect(x: number, y: number, width: number, height: number, radius: number): this {
    const w = Math.max(0, width);
    const h = Math.max(0, height);
    this.context.beginPath();
    this.context.roundRect(x, y, w, h, Math.max(0, Math.min(radius, w / 2, h / 2)));
    return this;
  }

  circle(x: number, y: number, radius: number): this {
    this.context.beginPath();
    this.context.arc(x, y, Math.max(0, radius), 0, Math.PI * 2);
    return this;
  }

  /** The points `points` (x and y one after the other) joined, and back to the first if `close`. */
  poly(points: number[], close = true): this {
    this.context.beginPath();
    for (let i = 0; i + 1 < points.length; i += 2) {
      if (i === 0) this.context.moveTo(points[i], points[i + 1]);
      else this.context.lineTo(points[i], points[i + 1]);
    }
    if (close) this.context.closePath();
    return this;
  }

  fill(paint: Paint): this {
    this.context.fillStyle = css(paint);
    this.context.fill();
    return this;
  }

  stroke(line: { width: number; color: number; alpha?: number }): this {
    this.context.lineWidth = line.width;
    this.context.strokeStyle = css(line);
    this.context.stroke();
    return this;
  }
}

interface Words {
  words: string;
  x: number;
  y: number;
  color: number;
  size: number;
  anchor: number;
}

/**
 * A canvas filling `host`, drawn again only when asked; what moves on its own (the playhead)
 * is drawn over it, alone, on a second canvas. Each pixel of them is one of the screen's, so
 * that a scale such as 115 % blurs nothing.
 */
export class Surface {
  readonly shapes: Pen;
  readonly over: Pen;
  private readonly base: HTMLCanvasElement;
  private readonly top: HTMLCanvasElement;
  /** The words, written over every shape once they are drawn. */
  private readonly words: Words[] = [];
  private wanted = true;
  private wantedOver = true;
  private destroyed = false;
  private frameId = 0;
  private readonly observer: ResizeObserver;
  private readonly fontsLoaded = () => this.redraw();
  /** The host's size in the screen's own pixels, as the browser lays it out. */
  private pixels: { width: number; height: number } | null = null;
  /** The size drawn in, in CSS pixels: the host's, to a fraction of one. */
  width = 0;
  height = 0;

  constructor(
    private readonly host: HTMLElement,
    private readonly paint: (surface: Surface) => void,
    private readonly paintOver: (surface: Surface) => void = () => undefined,
  ) {
    this.base = this.canvas();
    this.top = this.canvas();
    this.top.style.position = 'absolute';
    this.top.style.inset = '0';
    this.top.style.pointerEvents = 'none';
    this.shapes = new Pen(this.base.getContext('2d')!);
    this.over = new Pen(this.top.getContext('2d')!);
    this.observer = new ResizeObserver((entries) => {
      const box = entries[0]?.devicePixelContentBoxSize?.[0];
      this.pixels = box ? { width: box.inlineSize, height: box.blockSize } : null;
      this.redraw();
    });
  }

  private canvas(): HTMLCanvasElement {
    const canvas = document.createElement('canvas');
    canvas.style.display = 'block';
    canvas.style.width = '100%';
    canvas.style.height = '100%';
    return canvas;
  }

  async start(): Promise<void> {
    if (this.destroyed) return;
    if (getComputedStyle(this.host).position === 'static') this.host.style.position = 'relative';
    this.host.append(this.base, this.top);
    try {
      this.observer.observe(this.host, { box: 'device-pixel-content-box' });
    } catch {
      this.observer.observe(this.host);
    }
    // the words written again in their own font once it has come
    document.fonts?.addEventListener('loadingdone', this.fontsLoaded);
    this.frame();
  }

  /** Draws what is wanted, once a frame at most. */
  private frame(): void {
    if (this.destroyed) return;
    if (this.wanted) {
      this.wanted = false;
      this.wantedOver = true;
      this.fit();
      this.words.length = 0;
      this.draw(this.shapes, this.base, () => this.paint(this));
      this.writeWords();
    }
    if (this.wantedOver) {
      this.wantedOver = false;
      this.draw(this.over, this.top, () => this.paintOver(this));
    }
    this.frameId = requestAnimationFrame(() => this.frame());
  }

  /** Both canvases made as large as the host, in the screen's pixels. */
  private fit(): void {
    const ratio = window.devicePixelRatio || 1;
    const pixels = this.pixels ?? {
      width: Math.round(this.host.clientWidth * ratio),
      height: Math.round(this.host.clientHeight * ratio),
    };
    this.width = pixels.width / ratio;
    this.height = pixels.height / ratio;
    for (const canvas of [this.base, this.top]) {
      if (canvas.width !== pixels.width) canvas.width = pixels.width;
      if (canvas.height !== pixels.height) canvas.height = pixels.height;
    }
    this.shapes.ratio = this.over.ratio = ratio;
  }

  private draw(pen: Pen, canvas: HTMLCanvasElement, paint: () => void): void {
    const context = pen.context;
    context.setTransform(1, 0, 0, 1, 0, 0);
    context.clearRect(0, 0, canvas.width, canvas.height);
    context.setTransform(pen.ratio, 0, 0, pen.ratio, 0, 0);
    paint();
  }

  private writeWords(): void {
    const context = this.shapes.context;
    const r = this.shapes.ratio;
    context.textBaseline = 'top';
    for (const w of this.words) {
      context.font = `${w.size}px ${FONT}`;
      context.fillStyle = css(w.color);
      context.textAlign = w.anchor >= 1 ? 'right' : w.anchor > 0 ? 'center' : 'left';
      // on the screen's pixels, the words drawn as sharp as they were written
      context.fillText(w.words, Math.round(w.x * r) / r, Math.round(w.y * r) / r);
    }
  }

  redraw(): void {
    this.wanted = true;
  }

  redrawOver(): void {
    this.wantedOver = true;
  }

  /** Words at `x` and `y`, their top there, written over every shape drawn. */
  text(words: string, x: number, y: number, color = 0xbcc2cf, size = 11, anchor = 0): void {
    this.words.push({ words, x, y, color, size, anchor });
  }

  destroy(): void {
    this.destroyed = true;
    cancelAnimationFrame(this.frameId);
    this.observer.disconnect();
    document.fonts?.removeEventListener('loadingdone', this.fontsLoaded);
    this.base.remove();
    this.top.remove();
  }
}
