'use client';

import { useEffect, useRef, useState } from 'react';
import {
  editThumbnailBgUrl,
  fetchEditFrame,
  generateEditTextStyle,
  restyleEditFrame,
  saveEditThumbnail,
} from '@/lib/api';
import type { ThumbnailSpec, ThumbnailTextStyle } from '@/lib/types';

const W = 1280;
const H = 720;

type Align = 'left' | 'center' | 'right';

const DEFAULT_STYLE: ThumbnailTextStyle = {
  fill: '#ffffff',
  gradient: null,
  outlineColor: '#000000',
  outlineWidth: 10,
  shadowColor: '#000000',
  shadowBlur: 0,
  shadowOffsetY: 0,
  highlight: null,
};

interface ThumbnailEditorProps {
  editId: number;
  duration: number;
  suggestedTexts: string[];
  /** Previously-saved thumbnail state for this run (rehydrated on reopen). */
  saved?: ThumbnailSpec | null;
  /** Topic/title context, used to make AI text styling more relevant. */
  context?: string;
  /** Called with the saved spec after a successful "Save to folder", so the
   *  parent can reflect it immediately (e.g. the "✓ saved" badge). */
  onSaved?: (spec: ThumbnailSpec) => void;
}

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, v));
}

function wrapLines(ctx: CanvasRenderingContext2D, text: string, maxWidth: number): string[] {
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

function roundRectPath(
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

/** Render the background still (cover-fit) onto an offscreen canvas and return a
 *  PNG data URL — persisted so an AI restyle / exact frame restores on reopen. */
function renderBackground(img: HTMLImageElement): string | undefined {
  const off = document.createElement('canvas');
  off.width = W;
  off.height = H;
  const ctx = off.getContext('2d');
  if (!ctx) return undefined;
  const scale = Math.max(W / img.width, H / img.height);
  const dw = img.width * scale;
  const dh = img.height * scale;
  ctx.drawImage(img, (W - dw) / 2, (H - dh) / 2, dw, dh);
  return off.toDataURL('image/png');
}

export default function ThumbnailEditor({
  editId,
  duration,
  suggestedTexts,
  saved,
  context,
  onSaved,
}: ThumbnailEditorProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const reqIdRef = useRef(0);
  const draggingRef = useRef(false);
  const [frame, setFrame] = useState<HTMLImageElement | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Text + layout (rehydrated from a saved spec when present).
  const [text, setText] = useState(() => saved?.text ?? suggestedTexts[0] ?? '');
  const [fontSize, setFontSize] = useState(() => saved?.fontSize ?? 110);
  const [uppercase, setUppercase] = useState(() => saved?.uppercase ?? true);
  const [align, setAlign] = useState<Align>(() => saved?.align ?? 'center');
  const [posX, setPosX] = useState(() => saved?.posX ?? 0.5);
  const [posY, setPosY] = useState(() => saved?.posY ?? 0.82);
  const [style, setStyle] = useState<ThumbnailTextStyle>(() =>
    saved?.style ? { ...DEFAULT_STYLE, ...saved.style } : DEFAULT_STYLE
  );

  const [t, setT] = useState(() => saved?.frameTime ?? clamp(duration / 3, 0, duration));
  const [restyled, setRestyled] = useState(() => saved?.restyled ?? false);

  const [restyling, setRestyling] = useState(false);
  const [restylePrompt, setRestylePrompt] = useState(
    'Enhance this into a punchy YouTube thumbnail: richer contrast and saturation, cinematic color grade, sharper detail, brighter highlights. Keep the subject and composition unchanged. No text.'
  );

  const [styling, setStyling] = useState(false);
  const [textStylePrompt, setTextStylePrompt] = useState('');

  const [saving, setSaving] = useState(false);
  const [savedPath, setSavedPath] = useState<string | null>(null);

  // Latest request wins: while dragging the frame slider, stale frames are
  // discarded.
  const loadFrame = async (loader: () => Promise<Blob>) => {
    const myReq = ++reqIdRef.current;
    setLoading(true);
    setError(null);
    try {
      const blob = await loader();
      if (myReq !== reqIdRef.current) return;
      const url = URL.createObjectURL(blob);
      const img = new Image();
      img.onload = () => {
        URL.revokeObjectURL(url);
        if (myReq !== reqIdRef.current) return;
        setFrame(img);
        setLoading(false);
      };
      img.onerror = () => {
        URL.revokeObjectURL(url);
        if (myReq !== reqIdRef.current) return;
        setError('Could not load the frame image.');
        setLoading(false);
      };
      img.src = url;
    } catch (e: any) {
      if (myReq !== reqIdRef.current) return;
      setError(e.message || 'Failed to load frame');
      setLoading(false);
    }
  };

  // On open: restore the saved background still (the possibly AI-restyled image),
  // falling back to the original frame at the saved time; otherwise grab the
  // default frame. Runs once.
  const hydratedRef = useRef(false);
  useEffect(() => {
    if (hydratedRef.current) return;
    hydratedRef.current = true;
    if (saved) {
      loadFrame(async () => {
        const res = await fetch(editThumbnailBgUrl(editId, saved.frameTime), {
          cache: 'no-store',
        });
        if (res.ok) return res.blob();
        // No saved background on disk — re-grab the original frame.
        return fetchEditFrame(editId, saved.frameTime);
      });
    } else {
      loadFrame(() => fetchEditFrame(editId, t));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Live frame preview: debounce slider changes. Skips the very first run (the
  // initial frame is loaded by the hydration effect above).
  const firstTRef = useRef(true);
  useEffect(() => {
    if (firstTRef.current) {
      firstTRef.current = false;
      return;
    }
    setRestyled(false); // picking a new frame drops any AI restyle
    const id = setTimeout(() => loadFrame(() => fetchEditFrame(editId, t)), 160);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [t, editId]);

  // Redraw whenever the frame or any text/layout/style changes.
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.clearRect(0, 0, W, H);
    if (frame) {
      const scale = Math.max(W / frame.width, H / frame.height);
      const dw = frame.width * scale;
      const dh = frame.height * scale;
      ctx.drawImage(frame, (W - dw) / 2, (H - dh) / 2, dw, dh);
    } else {
      ctx.fillStyle = '#111827';
      ctx.fillRect(0, 0, W, H);
    }

    const value = uppercase ? text.toUpperCase() : text;
    if (!value.trim()) return;

    ctx.font = `900 ${fontSize}px Arial, "Arial Black", sans-serif`;
    ctx.textAlign = align;
    ctx.textBaseline = 'middle';
    ctx.lineJoin = 'round';

    const lines = wrapLines(ctx, value, W * 0.92);
    const lineH = fontSize * 1.16;
    const total = lines.length * lineH;
    const anchorX = posX * W;
    let y = posY * H - total / 2 + lineH / 2;

    const band = style.highlight;
    for (const line of lines) {
      if (line.trim()) {
        const tw = ctx.measureText(line).width;
        const lineLeft =
          align === 'left' ? anchorX : align === 'right' ? anchorX - tw : anchorX - tw / 2;

        // Highlight band behind the text (carries the drop shadow when set).
        if (band) {
          const padX = fontSize * 0.26;
          const bh = fontSize * 1.04;
          ctx.save();
          if (style.shadowBlur > 0) {
            ctx.shadowColor = style.shadowColor;
            ctx.shadowBlur = style.shadowBlur;
            ctx.shadowOffsetY = style.shadowOffsetY;
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
        if (!band && style.shadowBlur > 0) {
          ctx.save();
          ctx.shadowColor = style.shadowColor;
          ctx.shadowBlur = style.shadowBlur;
          ctx.shadowOffsetY = style.shadowOffsetY;
          ctx.fillStyle = typeof paint === 'string' ? paint : style.fill;
          ctx.fillText(line, anchorX, y);
          ctx.restore();
        }

        if (style.outlineWidth > 0) {
          ctx.lineWidth = style.outlineWidth * 2;
          ctx.strokeStyle = style.outlineColor;
          ctx.strokeText(line, anchorX, y);
        }
        ctx.fillStyle = paint;
        ctx.fillText(line, anchorX, y);
      }
      y += lineH;
    }
  }, [frame, text, fontSize, uppercase, align, posX, posY, style]);

  // Drag anywhere on the canvas to move the text anchor.
  const setPosFromEvent = (clientX: number, clientY: number) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;
    setPosX(clamp((clientX - rect.left) / rect.width, 0.02, 0.98));
    setPosY(clamp((clientY - rect.top) / rect.height, 0.05, 0.95));
  };
  const handlePointerDown = (e: React.PointerEvent<HTMLCanvasElement>) => {
    draggingRef.current = true;
    canvasRef.current?.setPointerCapture(e.pointerId);
    setPosFromEvent(e.clientX, e.clientY);
  };
  const handlePointerMove = (e: React.PointerEvent<HTMLCanvasElement>) => {
    if (draggingRef.current) setPosFromEvent(e.clientX, e.clientY);
  };
  const handlePointerUp = (e: React.PointerEvent<HTMLCanvasElement>) => {
    draggingRef.current = false;
    canvasRef.current?.releasePointerCapture(e.pointerId);
  };

  const handleRestyle = async () => {
    setRestyling(true);
    setError(null);
    try {
      await loadFrame(() => restyleEditFrame(editId, t, restylePrompt));
      setRestyled(true);
    } catch (e: any) {
      setError(e.message || 'AI restyle failed');
    } finally {
      setRestyling(false);
    }
  };

  const handleAiTextStyle = async () => {
    if (!text.trim()) {
      setError('Add some thumbnail text first, then style it.');
      return;
    }
    setStyling(true);
    setError(null);
    try {
      const s = await generateEditTextStyle(editId, {
        text,
        context,
        prompt: textStylePrompt,
      });
      const merged: ThumbnailTextStyle = { ...DEFAULT_STYLE, ...s };
      // The band's intended text color becomes the actual fill, so the "Text"
      // color control and the renderer stay consistent (band = color only).
      if (merged.highlight?.textColor) merged.fill = merged.highlight.textColor;
      setStyle(merged);
    } catch (e: any) {
      setError(e.message || 'AI text style failed');
    } finally {
      setStyling(false);
    }
  };

  const handleDownload = () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    setError(null);
    try {
      canvas.toBlob((blob) => {
        if (!blob) {
          setError('Could not render the image. Try "Save to folder" instead.');
          return;
        }
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = 'thumbnail.png';
        // The anchor must be in the DOM for .click() to trigger a download in
        // most WebViews/browsers.
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        setTimeout(() => URL.revokeObjectURL(url), 1000);
      }, 'image/png');
    } catch (e: any) {
      setError(e.message || 'Download failed. Use "Save to folder" instead.');
    }
  };

  const handleSave = async () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    setSaving(true);
    setSavedPath(null);
    setError(null);
    try {
      const spec: ThumbnailSpec = {
        text,
        fontSize,
        uppercase,
        align,
        posX,
        posY,
        frameTime: t,
        restyled,
        style,
      };
      const { path } = await saveEditThumbnail(editId, {
        image: canvas.toDataURL('image/png'),
        background: frame ? renderBackground(frame) : undefined,
        spec,
      });
      setSavedPath(path);
      onSaved?.(spec);
    } catch (e: any) {
      setError(e.message || 'Failed to save thumbnail');
    } finally {
      setSaving(false);
    }
  };

  const alignBtn = (a: Align, label: string) => (
    <button
      type="button"
      onClick={() => setAlign(a)}
      className={`px-2 py-1 text-xs rounded ${
        align === a
          ? 'bg-primary-600 text-white'
          : 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-200'
      }`}
      title={`Align ${a}`}
    >
      {label}
    </button>
  );

  return (
    <div className="space-y-3">
      <canvas
        ref={canvasRef}
        width={W}
        height={H}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        style={{ touchAction: 'none' }}
        className="w-full h-auto rounded border border-gray-200 dark:border-gray-700 bg-black cursor-move"
      />
      <p className="text-xs text-gray-400 dark:text-gray-500 -mt-1">
        Tip: drag on the image to position the text.
      </p>

      {/* Frame picker */}
      <div className="flex items-center gap-3">
        <span className="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap w-28">
          {loading ? 'Loading…' : `Frame at ${t.toFixed(1)}s`}
          {restyled && !loading && <span className="text-purple-500"> ✨</span>}
        </span>
        <input
          type="range"
          min={0}
          max={Math.max(0.1, duration)}
          step={0.1}
          value={t}
          onChange={(e) => setT(parseFloat(e.target.value))}
          className="flex-1"
        />
        <button
          type="button"
          onClick={handleRestyle}
          disabled={loading || restyling}
          className="btn btn-secondary text-xs whitespace-nowrap"
          title="Restyle this frame with the configured image model (requires an API key)"
        >
          {restyling ? 'Restyling…' : '✨ AI restyle frame'}
        </button>
      </div>

      {/* AI restyle prompt (optional) */}
      <details className="text-xs text-gray-500 dark:text-gray-400">
        <summary className="cursor-pointer select-none">✨ AI restyle prompt (optional)</summary>
        <textarea
          value={restylePrompt}
          onChange={(e) => setRestylePrompt(e.target.value)}
          rows={2}
          className="input text-sm mt-2"
          placeholder="How to restyle the frame (focus on background/lighting)."
        />
        <p className="mt-1">
          Tip: Gemini often refuses close-up shots of real faces. A wider frame or a
          background/lighting-focused prompt works best.
        </p>
      </details>

      {/* Text */}
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        rows={2}
        className="input text-sm"
        placeholder="Thumbnail text (use line breaks for multiple lines)"
      />
      {suggestedTexts.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {suggestedTexts.map((s, i) => (
            <button
              key={i}
              type="button"
              onClick={() => setText(s)}
              className="badge bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-200 text-xs"
              title="Use this suggestion"
            >
              {s}
            </button>
          ))}
        </div>
      )}

      {/* AI text styling */}
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onClick={handleAiTextStyle}
          disabled={styling}
          className="btn btn-secondary text-xs whitespace-nowrap"
          title="Let AI design an eye-catching text treatment (colors, gradient, shadow, highlight band)"
        >
          {styling ? 'Styling…' : '✨ AI style text'}
        </button>
        <button
          type="button"
          onClick={() => setStyle(DEFAULT_STYLE)}
          className="btn btn-secondary text-xs"
          title="Reset to plain white text with a black outline"
        >
          Reset style
        </button>
        <span className="text-xs text-gray-400 dark:text-gray-500">
          Styles the text only — the words stay crisp and editable.
        </span>
      </div>
      <details className="text-xs text-gray-500 dark:text-gray-400">
        <summary className="cursor-pointer select-none">✨ AI text-style direction (optional)</summary>
        <input
          value={textStylePrompt}
          onChange={(e) => setTextStylePrompt(e.target.value)}
          className="input text-sm mt-2"
          placeholder='e.g. "bold red and yellow, aggressive" or "clean minimal white"'
        />
      </details>

      {/* Sliders */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 items-center">
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-2">
          Size
          <input type="range" min={40} max={220} step={2} value={fontSize} onChange={(e) => setFontSize(parseInt(e.target.value))} className="flex-1" />
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-2">
          Outline
          <input
            type="range"
            min={0}
            max={24}
            step={1}
            value={style.outlineWidth}
            onChange={(e) => setStyle((s) => ({ ...s, outlineWidth: parseInt(e.target.value) }))}
            className="flex-1"
          />
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-2">
          X
          <input type="range" min={0.02} max={0.98} step={0.01} value={posX} onChange={(e) => setPosX(parseFloat(e.target.value))} className="flex-1" />
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-2">
          Y
          <input type="range" min={0.05} max={0.95} step={0.01} value={posY} onChange={(e) => setPosY(parseFloat(e.target.value))} className="flex-1" />
        </label>
      </div>

      {/* Alignment + colors + caps */}
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        <div className="flex items-center gap-1">
          <span className="text-xs text-gray-500 dark:text-gray-400 mr-1">Align</span>
          {alignBtn('left', '⯇')}
          {alignBtn('center', '☰')}
          {alignBtn('right', '⯈')}
        </div>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1">
          Text
          <input
            type="color"
            value={style.fill}
            onChange={(e) => setStyle((s) => ({ ...s, fill: e.target.value, gradient: null }))}
            className="w-7 h-7 p-0 border-0 bg-transparent"
          />
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1">
          Edge
          <input
            type="color"
            value={style.outlineColor}
            onChange={(e) => setStyle((s) => ({ ...s, outlineColor: e.target.value }))}
            className="w-7 h-7 p-0 border-0 bg-transparent"
          />
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1 cursor-pointer">
          <input type="checkbox" checked={uppercase} onChange={(e) => setUppercase(e.target.checked)} className="rounded" />
          CAPS
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1 cursor-pointer">
          <input
            type="checkbox"
            checked={!!style.highlight}
            onChange={(e) =>
              setStyle((s) => ({
                ...s,
                highlight: e.target.checked
                  ? { color: s.highlight?.color ?? '#e11d2a', textColor: s.fill }
                  : null,
              }))
            }
            className="rounded"
          />
          Band
          {style.highlight && (
            <input
              type="color"
              value={style.highlight.color}
              onChange={(e) =>
                setStyle((s) => ({
                  ...s,
                  highlight: { color: e.target.value, textColor: s.highlight?.textColor ?? s.fill },
                }))
              }
              className="w-7 h-7 p-0 border-0 bg-transparent"
            />
          )}
        </label>
        <label className="text-xs text-gray-500 dark:text-gray-400 flex items-center gap-1 cursor-pointer">
          <input
            type="checkbox"
            checked={style.shadowBlur > 0}
            onChange={(e) =>
              setStyle((s) => ({
                ...s,
                shadowBlur: e.target.checked ? 18 : 0,
                shadowOffsetY: e.target.checked ? 6 : 0,
                shadowColor: s.shadowColor || '#000000',
              }))
            }
            className="rounded"
          />
          Shadow
        </label>
      </div>

      {error && (
        <div className="p-2 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded text-sm text-red-600 dark:text-red-400">
          {error}
        </div>
      )}
      {savedPath && (
        <p className="text-xs text-green-600 dark:text-green-400 break-all">Saved: {savedPath}</p>
      )}

      <div className="flex items-center gap-2">
        <button onClick={handleSave} disabled={saving} className="btn btn-primary text-sm">
          {saving ? 'Saving…' : 'Save to folder'}
        </button>
        <button onClick={handleDownload} className="btn btn-secondary text-sm">
          Download PNG
        </button>
      </div>
    </div>
  );
}
