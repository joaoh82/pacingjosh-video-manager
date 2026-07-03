/**
 * Shared styled-caption rendering, extracted from the thumbnail builder so the
 * thumbnail canvas, the video text-overlay live preview, and the exported
 * transparent-PNG overlay all draw text identically (guaranteeing WYSIWYG).
 *
 * The draw routine only paints the text (+ optional highlight band) — the caller
 * is responsible for any background. All metrics are in the target canvas's own
 * pixel space; a `scale` multiplier keeps the same look across differently-sized
 * canvases (e.g. a 720-tall thumbnail vs a 1080- or 2160-tall video frame).
 */

import type { TextOverlaySpec, ThumbnailTextStyle } from './types';

export type Align = 'left' | 'center' | 'right';

/** Reference frame height that a `TextOverlaySpec`'s px metrics are authored
 *  against. Rendering to a real frame scales everything by `frameHeight / this`. */
export const OVERLAY_REF_HEIGHT = 1080;

/** Sensible default caption treatment: white fill, black outline, no shadow. */
export const DEFAULT_TEXT_STYLE: ThumbnailTextStyle = {
  fill: '#ffffff',
  gradient: null,
  outlineColor: '#000000',
  outlineWidth: 10,
  shadowColor: '#000000',
  shadowBlur: 0,
  shadowOffsetY: 0,
  highlight: null,
};

export interface StyledTextParams {
  text: string;
  /** Font size in px, in the canvas's own pixel space (before `scale`). */
  fontSize: number;
  uppercase: boolean;
  align: Align;
  /** Horizontal anchor, 0–1 of width. */
  posX: number;
  /** Vertical anchor, 0–1 of height. */
  posY: number;
  style: ThumbnailTextStyle;
}

/** Word-wrap `text` to `maxWidth` using the ctx's current font. Honors explicit
 *  newlines. */
export function wrapLines(
  ctx: CanvasRenderingContext2D,
  text: string,
  maxWidth: number
): string[] {
  const out: string[] = [];
  for (const para of text.split('\n')) {
    const words = para.split(/\s+/).filter(Boolean);
    if (words.length === 0) {
      out.push('');
      continue;
    }
    let line = words[0];
    for (let i = 1; i < words.length; i++) {
      const test = `${line} ${words[i]}`;
      if (ctx.measureText(test).width > maxWidth) {
        out.push(line);
        line = words[i];
      } else {
        line = test;
      }
    }
    out.push(line);
  }
  return out;
}

export function roundRectPath(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number
) {
  const rr = Math.max(0, Math.min(r, w / 2, h / 2));
  ctx.beginPath();
  ctx.moveTo(x + rr, y);
  ctx.arcTo(x + w, y, x + w, y + h, rr);
  ctx.arcTo(x + w, y + h, x, y + h, rr);
  ctx.arcTo(x, y + h, x, y, rr);
  ctx.arcTo(x, y, x + w, y, rr);
  ctx.closePath();
}

/**
 * Draw styled caption text onto `ctx` across a `width`×`height` region. Only the
 * text (and an optional highlight band) is painted — no background. `scale`
 * multiplies the absolute px metrics (font size, outline, shadow) so the same
 * treatment renders proportionally on any canvas size; pass 1 for a canvas
 * authored in the same pixel space as `params`.
 */
export function drawStyledText(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
  params: StyledTextParams,
  scale = 1
): void {
  const value = params.uppercase ? params.text.toUpperCase() : params.text;
  if (!value.trim()) return;

  const style = params.style;
  const fontSize = params.fontSize * scale;
  const outlineWidth = style.outlineWidth * scale;
  const shadowBlur = style.shadowBlur * scale;
  const shadowOffsetY = style.shadowOffsetY * scale;

  ctx.font = `900 ${fontSize}px Arial, "Arial Black", sans-serif`;
  ctx.textAlign = params.align;
  ctx.textBaseline = 'middle';
  ctx.lineJoin = 'round';

  const lines = wrapLines(ctx, value, width * 0.92);
  const lineH = fontSize * 1.16;
  const total = lines.length * lineH;
  const anchorX = params.posX * width;
  let y = params.posY * height - total / 2 + lineH / 2;

  const band = style.highlight;
  for (const line of lines) {
    if (line.trim()) {
      const tw = ctx.measureText(line).width;
      const lineLeft =
        params.align === 'left'
          ? anchorX
          : params.align === 'right'
          ? anchorX - tw
          : anchorX - tw / 2;

      // Highlight band behind the text (carries the drop shadow when set).
      if (band) {
        const padX = fontSize * 0.26;
        const bh = fontSize * 1.04;
        ctx.save();
        if (shadowBlur > 0) {
          ctx.shadowColor = style.shadowColor;
          ctx.shadowBlur = shadowBlur;
          ctx.shadowOffsetY = shadowOffsetY;
        }
        ctx.fillStyle = band.color;
        roundRectPath(ctx, lineLeft - padX, y - bh / 2, tw + padX * 2, bh, bh * 0.16);
        ctx.fill();
        ctx.restore();
      }

      // Text paint: gradient overrides the solid fill.
      let paint: string | CanvasGradient = style.fill;
      if (style.gradient) {
        const g = ctx.createLinearGradient(0, y - fontSize * 0.6, 0, y + fontSize * 0.6);
        g.addColorStop(0, style.gradient.from);
        g.addColorStop(1, style.gradient.to);
        paint = g;
      }

      // Soft shadow under the glyphs (only when there's no band to carry it).
      if (!band && shadowBlur > 0) {
        ctx.save();
        ctx.shadowColor = style.shadowColor;
        ctx.shadowBlur = shadowBlur;
        ctx.shadowOffsetY = shadowOffsetY;
        ctx.fillStyle = typeof paint === 'string' ? paint : style.fill;
        ctx.fillText(line, anchorX, y);
        ctx.restore();
      }

      if (outlineWidth > 0) {
        ctx.lineWidth = outlineWidth * 2;
        ctx.strokeStyle = style.outlineColor;
        ctx.strokeText(line, anchorX, y);
      }
      ctx.fillStyle = paint;
      ctx.fillText(line, anchorX, y);
    }
    y += lineH;
  }
}

/** Draw a `TextOverlaySpec` onto `ctx` at a real frame size, scaling its
 *  1080-referenced metrics to `height`. Used for both the live preview and the
 *  exported PNG so they match exactly. */
export function drawTextOverlay(
  ctx: CanvasRenderingContext2D,
  width: number,
  height: number,
  spec: TextOverlaySpec
): void {
  drawStyledText(
    ctx,
    width,
    height,
    {
      text: spec.text,
      fontSize: spec.fontSize,
      uppercase: spec.uppercase,
      align: spec.align,
      posX: spec.posX,
      posY: spec.posY,
      style: spec.style,
    },
    height / OVERLAY_REF_HEIGHT
  );
}

/** Rasterize a text overlay to a transparent PNG data URL at the given output
 *  resolution (the real video frame size), for upload + compositing. */
export function rasterizeTextOverlayPng(
  spec: TextOverlaySpec,
  width: number,
  height: number
): string {
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(2, Math.round(width));
  canvas.height = Math.max(2, Math.round(height));
  const ctx = canvas.getContext('2d');
  if (!ctx) return '';
  drawTextOverlay(ctx, canvas.width, canvas.height, spec);
  return canvas.toDataURL('image/png');
}
